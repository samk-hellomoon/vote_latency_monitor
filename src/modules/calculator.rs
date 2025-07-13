//! Latency Calculator Module
//!
//! This module is responsible for calculating vote latency metrics.
//! It processes parsed vote transactions and computes various latency
//! measurements including network propagation time and statistical aggregations.

use crate::error::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast};
use tokio::select;
use tracing::{info, trace};

use crate::Config;
use crate::models::{LatencyMetrics, VoteLatency};
use crate::modules::{Shutdown, ShutdownSignal};

/// Trait for latency calculation implementations
#[async_trait]
pub trait LatencyCalculatorTrait: Send + Sync {
    /// Calculate latency for a vote
    async fn calculate(&self, vote: &VoteLatency) -> Result<LatencyMetrics>;
    
    /// Get aggregated metrics for a validator
    async fn get_validator_metrics(&self, pubkey: &Pubkey) -> Option<LatencyMetrics>;
    
    /// Get global metrics across all validators
    async fn get_global_metrics(&self) -> LatencyMetrics;
}

/// Latency calculator implementation
pub struct LatencyCalculator {
    /// Window size for moving averages
    window_size: usize,
    /// Per-validator metrics
    validator_metrics: Arc<DashMap<Pubkey, ValidatorMetricsData>>,
    /// Global metrics
    global_metrics: Arc<RwLock<GlobalMetricsData>>,
    /// Configuration
    config: Arc<Config>,
    /// Storage manager
    storage: Option<Arc<dyn crate::modules::storage::StorageManagerTrait>>,
    /// Shutdown receiver
    shutdown_rx: Option<broadcast::Receiver<ShutdownSignal>>,
    /// Task handle
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Data structure for tracking per-validator metrics
struct ValidatorMetricsData {
    latencies: VecDeque<u64>,
    slot_latencies: VecDeque<Vec<u8>>,  // Store slot-based latencies
    total_votes: u64,
    last_update: chrono::DateTime<chrono::Utc>,
}

/// Data structure for tracking global metrics
struct GlobalMetricsData {
    all_latencies: VecDeque<u64>,
    all_slot_latencies: VecDeque<Vec<u8>>,  // Store all slot latencies
    total_votes: u64,
    validator_count: usize,
    current_metrics: Option<LatencyMetrics>,
}

impl Default for GlobalMetricsData {
    fn default() -> Self {
        Self {
            all_latencies: VecDeque::new(),
            all_slot_latencies: VecDeque::new(),
            total_votes: 0,
            validator_count: 0,
            current_metrics: None,
        }
    }
}

impl LatencyCalculator {
    /// Create a new latency calculator
    pub async fn new(
        config: Arc<Config>,
        storage: Option<Arc<dyn crate::modules::storage::StorageManagerTrait>>,
        shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    ) -> Result<Self> {
        let window_size = config.latency.window_size;
        Ok(Self {
            window_size,
            validator_metrics: Arc::new(DashMap::new()),
            global_metrics: Arc::new(RwLock::new(GlobalMetricsData {
                all_latencies: VecDeque::with_capacity(window_size),
                all_slot_latencies: VecDeque::with_capacity(window_size),
                total_votes: 0,
                validator_count: 0,
                current_metrics: None,
            })),
            config,
            storage,
            shutdown_rx: Some(shutdown_rx),
            task_handle: None,
        })
    }

    /// Update metrics with a new vote latency
    async fn update_metrics(&self, vote: &VoteLatency) -> Result<()> {
        trace!("Updating metrics for validator: {}", vote.validator_pubkey);
        
        // Update per-validator metrics
        self.validator_metrics
            .entry(vote.validator_pubkey.clone())
            .and_modify(|data| {
                data.latencies.push_back(vote.latency_ms);
                data.slot_latencies.push_back(vote.latency_slots.clone());
                if data.latencies.len() > self.window_size {
                    data.latencies.pop_front();
                }
                if data.slot_latencies.len() > self.window_size {
                    data.slot_latencies.pop_front();
                }
                data.total_votes += 1;
                data.last_update = chrono::Utc::now();
            })
            .or_insert_with(|| {
                let mut latencies = VecDeque::with_capacity(self.window_size);
                let mut slot_latencies = VecDeque::with_capacity(self.window_size);
                latencies.push_back(vote.latency_ms);
                slot_latencies.push_back(vote.latency_slots.clone());
                ValidatorMetricsData {
                    latencies,
                    slot_latencies,
                    total_votes: 1,
                    last_update: chrono::Utc::now(),
                }
            });

        // Update global metrics
        let mut global = self.global_metrics.write().await;
        global.all_latencies.push_back(vote.latency_ms);
        global.all_slot_latencies.push_back(vote.latency_slots.clone());
        if global.all_latencies.len() > self.window_size * 10 {
            global.all_latencies.pop_front();
        }
        if global.all_slot_latencies.len() > self.window_size * 10 {
            global.all_slot_latencies.pop_front();
        }
        global.total_votes += 1;
        global.validator_count = self.validator_metrics.len();

        Ok(())
    }

    /// Calculate statistics from a collection of latencies
    fn calculate_stats(latencies: &[u64]) -> LatencyMetrics {
        if latencies.is_empty() {
            return LatencyMetrics::default();
        }

        let sum: u64 = latencies.iter().sum();
        let mean = sum as f64 / latencies.len() as f64;

        let mut sorted = latencies.to_vec();
        sorted.sort_unstable();

        let median = if sorted.len() % 2 == 0 {
            let mid = sorted.len() / 2;
            (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
        } else {
            sorted[sorted.len() / 2] as f64
        };

        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;

        LatencyMetrics {
            mean_ms: mean,
            median_ms: median,
            p95_ms: sorted.get(p95_idx).copied().unwrap_or(0) as f64,
            p99_ms: sorted.get(p99_idx).copied().unwrap_or(0) as f64,
            min_ms: *sorted.first().unwrap() as f64,
            max_ms: *sorted.last().unwrap() as f64,
            sample_count: latencies.len() as u64,
            timestamp: chrono::Utc::now(),
            // Slot-based metrics will be filled by calculate_slot_stats
            mean_slots: 0.0,
            median_slots: 0.0,
            p95_slots: 0.0,
            p99_slots: 0.0,
            min_slots: 0.0,
            max_slots: 0.0,
            votes_1_slot: 0,
            votes_2_slots: 0,
            votes_3plus_slots: 0,
        }
    }
    
    /// Calculate slot-based statistics from slot latency data
    fn calculate_slot_stats(slot_latencies: &[Vec<u8>]) -> (f32, f32, f32, f32, f32, f32, u64, u64, u64) {
        if slot_latencies.is_empty() {
            return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0);
        }
        
        // Flatten all slot latencies into a single vector
        let mut all_latencies: Vec<u8> = Vec::new();
        let mut votes_1_slot = 0u64;
        let mut votes_2_slots = 0u64;
        let mut votes_3plus_slots = 0u64;
        
        for latencies in slot_latencies {
            for &latency in latencies {
                all_latencies.push(latency);
                match latency {
                    1 => votes_1_slot += 1,
                    2 => votes_2_slots += 1,
                    3..=255 => votes_3plus_slots += 1,
                    _ => {}, // 0 latency (same slot)
                }
            }
        }
        
        if all_latencies.is_empty() {
            return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, votes_1_slot, votes_2_slots, votes_3plus_slots);
        }
        
        // Calculate statistics
        let sum: u32 = all_latencies.iter().map(|&x| x as u32).sum();
        let mean = sum as f32 / all_latencies.len() as f32;
        
        let mut sorted = all_latencies.clone();
        sorted.sort_unstable();
        
        let median = if sorted.len() % 2 == 0 {
            let mid = sorted.len() / 2;
            (sorted[mid - 1] as f32 + sorted[mid] as f32) / 2.0
        } else {
            sorted[sorted.len() / 2] as f32
        };
        
        let p95_idx = ((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1);
        let p99_idx = ((sorted.len() as f32 * 0.99) as usize).min(sorted.len() - 1);
        
        let min = *sorted.first().unwrap() as f32;
        let max = *sorted.last().unwrap() as f32;
        let p95 = sorted[p95_idx] as f32;
        let p99 = sorted[p99_idx] as f32;
        
        (mean, median, p95, p99, min, max, votes_1_slot, votes_2_slots, votes_3plus_slots)
    }
    
    /// Calculate combined time and slot-based statistics
    fn calculate_combined_stats(latencies: &[u64], slot_latencies: &[Vec<u8>]) -> LatencyMetrics {
        let mut metrics = Self::calculate_stats(latencies);
        
        let (mean_slots, median_slots, p95_slots, p99_slots, min_slots, max_slots, 
             votes_1_slot, votes_2_slots, votes_3plus_slots) = Self::calculate_slot_stats(slot_latencies);
        
        metrics.mean_slots = mean_slots;
        metrics.median_slots = median_slots;
        metrics.p95_slots = p95_slots;
        metrics.p99_slots = p99_slots;
        metrics.min_slots = min_slots;
        metrics.max_slots = max_slots;
        metrics.votes_1_slot = votes_1_slot;
        metrics.votes_2_slots = votes_2_slots;
        metrics.votes_3plus_slots = votes_3plus_slots;
        
        metrics
    }

    /// Start background metrics aggregation task
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting latency calculator");
        
        // Start periodic metrics logging
        let validator_metrics = Arc::clone(&self.validator_metrics);
        let global_metrics = Arc::clone(&self.global_metrics);
        let mut shutdown_rx = self.shutdown_rx.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Shutdown receiver not initialized"))?
            .resubscribe();
        let storage = self.storage.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                select! {
                    _ = interval.tick() => {
                        // Quickly grab a snapshot of the data to minimize lock time
                        let (latencies, slot_latencies, validator_count) = {
                            let global = global_metrics.read().await;
                            let latencies: Vec<u64> = global.all_latencies.iter().copied().collect();
                            let slot_latencies: Vec<Vec<u8>> = global.all_slot_latencies.iter().cloned().collect();
                            (latencies, slot_latencies, validator_metrics.len())
                        };
                        
                        if !latencies.is_empty() {
                            let metrics = LatencyCalculator::calculate_combined_stats(&latencies, &slot_latencies);
                            info!(
                                "Global metrics - Mean: {:.2}ms ({:.2} slots), Median: {:.2}ms ({:.2} slots), P95: {:.2}ms ({:.2} slots), Validators: {}",
                                metrics.mean_ms, metrics.mean_slots,
                                metrics.median_ms, metrics.median_slots,
                                metrics.p95_ms, metrics.p95_slots,
                                validator_count
                            );
                            info!(
                                "Vote distribution - 1 slot: {}, 2 slots: {}, 3+ slots: {}",
                                metrics.votes_1_slot, metrics.votes_2_slots, metrics.votes_3plus_slots
                            );
                            
                            // Store metrics in a separate non-blocking task to avoid holding locks
                            if let Some(storage) = &storage {
                                let storage_clone = storage.clone();
                                let metrics_clone = metrics.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = storage_clone.store_metrics(&metrics_clone, None).await {
                                        tracing::error!("Failed to store global metrics: {}", e);
                                    }
                                });
                            }
                            
                            // Update current metrics with minimal lock time
                            let mut global = global_metrics.write().await;
                            global.current_metrics = Some(metrics);
                            drop(global); // Explicitly drop to release lock immediately
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Latency calculator metrics task received shutdown signal");
                        break;
                    }
                }
            }
        });
        
        self.task_handle = Some(handle);
        Ok(())
    }
}

#[async_trait]
impl Shutdown for LatencyCalculator {
    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down latency calculator");
        
        // Cancel the metrics task
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                handle
            ).await;
        }
        
        // Save final metrics if storage is available
        if let Some(storage) = &self.storage {
            let global = self.global_metrics.read().await;
            if let Some(metrics) = &global.current_metrics {
                if let Err(e) = storage.store_metrics(metrics, None).await {
                    tracing::error!("Failed to save final global metrics: {}", e);
                }
            }
        }
        
        info!("Latency calculator shutdown complete");
        Ok(())
    }
}

#[async_trait]
impl LatencyCalculatorTrait for LatencyCalculator {
    async fn calculate(&self, vote: &VoteLatency) -> Result<LatencyMetrics> {
        // Update internal metrics
        self.update_metrics(vote).await?;
        
        // Get validator's current metrics
        if let Some(data) = self.validator_metrics.get(&vote.validator_pubkey) {
            let latencies: Vec<u64> = data.latencies.iter().copied().collect();
            let slot_latencies: Vec<Vec<u8>> = data.slot_latencies.iter().cloned().collect();
            Ok(Self::calculate_combined_stats(&latencies, &slot_latencies))
        } else {
            Ok(LatencyMetrics::default())
        }
    }

    async fn get_validator_metrics(&self, pubkey: &Pubkey) -> Option<LatencyMetrics> {
        self.validator_metrics.get(pubkey).map(|data| {
            let latencies: Vec<u64> = data.latencies.iter().copied().collect();
            let slot_latencies: Vec<Vec<u8>> = data.slot_latencies.iter().cloned().collect();
            Self::calculate_combined_stats(&latencies, &slot_latencies)
        })
    }

    async fn get_global_metrics(&self) -> LatencyMetrics {
        let global = self.global_metrics.read().await;
        let latencies: Vec<u64> = global.all_latencies.iter().copied().collect();
        let slot_latencies: Vec<Vec<u8>> = global.all_slot_latencies.iter().cloned().collect();
        Self::calculate_combined_stats(&latencies, &slot_latencies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use tokio::sync::broadcast;

    #[test]
    fn test_calculate_stats() {
        let latencies = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let metrics = LatencyCalculator::calculate_stats(&latencies);
        
        assert_eq!(metrics.mean_ms, 55.0);
        assert_eq!(metrics.median_ms, 55.0);
        assert_eq!(metrics.min_ms, 10.0);
        assert_eq!(metrics.max_ms, 100.0);
        assert_eq!(metrics.sample_count, 10);
    }
    
    #[test]
    fn test_calculate_slot_stats() {
        let slot_latencies = vec![
            vec![1, 2, 3],
            vec![1, 1, 2],
            vec![2, 3, 4],
        ];
        
        let (mean, median, _p95, _p99, min, max, votes_1, votes_2, votes_3plus) = 
            LatencyCalculator::calculate_slot_stats(&slot_latencies);
        
        // We have 9 total latencies: [1, 2, 3, 1, 1, 2, 2, 3, 4]
        // Sorted: [1, 1, 1, 2, 2, 2, 3, 3, 4]
        assert_eq!(votes_1, 3);
        assert_eq!(votes_2, 3);
        assert_eq!(votes_3plus, 3);
        
        assert!((mean - 2.11).abs() < 0.01); // (1+1+1+2+2+2+3+3+4)/9 = 2.11
        assert_eq!(median, 2.0);
        assert_eq!(min, 1.0);
        assert_eq!(max, 4.0);
    }

    #[tokio::test]
    async fn test_latency_calculator() {
        let config = Arc::new(Config::default());
        let (_shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let calculator = LatencyCalculator::new(config, None, shutdown_rx).await.unwrap();
        
        let vote = VoteLatency {
            validator_pubkey: Pubkey::new_unique(),
            vote_pubkey: Pubkey::new_unique(),
            slot: 12345,
            vote_timestamp: chrono::Utc::now(),
            received_timestamp: chrono::Utc::now(),
            latency_ms: 50,
            signature: "test".to_string(),
            voted_on_slots: vec![12345],
            landed_slot: 12347,
            latency_slots: vec![2],
        };
        
        let metrics = calculator.calculate(&vote).await.unwrap();
        assert_eq!(metrics.mean_ms, 50.0);
        assert_eq!(metrics.mean_slots, 2.0);
        assert_eq!(metrics.sample_count, 1);
    }
}