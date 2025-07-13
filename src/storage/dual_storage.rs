//! Dual Storage Implementation for Migration
//!
//! This module provides a dual-write storage backend that writes to both
//! SQLite and InfluxDB during the migration period.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{debug, error, warn};

use crate::models::{LatencyMetrics, ValidatorInfo, VoteLatency};
use crate::modules::storage::StorageManagerTrait;
use crate::storage::InfluxDBStorage;

/// Dual storage implementation that writes to both SQLite and InfluxDB
pub struct DualStorage {
    /// SQLite storage (existing)
    sqlite: Arc<dyn StorageManagerTrait>,
    
    /// InfluxDB storage (new)
    influxdb: Arc<InfluxDBStorage>,
    
    /// Whether to fail on InfluxDB errors
    fail_on_influx_error: bool,
}

impl DualStorage {
    /// Create a new dual storage instance
    pub async fn new(
        sqlite: Arc<dyn StorageManagerTrait>,
        influxdb: Arc<InfluxDBStorage>,
        fail_on_influx_error: bool,
    ) -> Result<Self> {
        Ok(Self {
            sqlite,
            influxdb,
            fail_on_influx_error,
        })
    }
}

#[async_trait]
impl StorageManagerTrait for DualStorage {
    async fn initialize(&self) -> crate::error::Result<()> {
        // Both storages should already be initialized
        Ok(())
    }
    
    async fn store_vote_latency(&self, latency: &VoteLatency) -> crate::error::Result<()> {
        debug!(
            "Dual storage: writing vote latency for validator {}",
            latency.validator_pubkey
        );
        
        // Write to InfluxDB first (primary)
        match self.influxdb.store_vote_latency(latency).await {
            Ok(_) => debug!("Successfully wrote to InfluxDB"),
            Err(e) => {
                error!("Failed to write to InfluxDB: {}", e);
                if self.fail_on_influx_error {
                    return Err(e);
                }
            }
        }
        
        // Write to SQLite (backup)
        match self.sqlite.store_vote_latency(latency).await {
            Ok(_) => debug!("Successfully wrote to SQLite"),
            Err(e) => {
                warn!("Failed to write to SQLite: {}", e);
                // Don't fail if SQLite write fails during migration
            }
        }
        
        Ok(())
    }
    
    async fn store_metrics(
        &self,
        metrics: &LatencyMetrics,
        validator_pubkey: Option<&solana_sdk::pubkey::Pubkey>,
    ) -> crate::error::Result<()> {
        // Only store metrics in SQLite for now
        // InfluxDB calculates metrics on-the-fly via queries
        self.sqlite.store_metrics(metrics, validator_pubkey).await
    }
    
    async fn query_latencies(
        &self,
        validator_pubkey: Option<&solana_sdk::pubkey::Pubkey>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> crate::error::Result<Vec<VoteLatency>> {
        // Query from InfluxDB if available, fallback to SQLite
        let validator_str = validator_pubkey.map(|p| p.to_string());
        match self.influxdb.query_latencies(validator_str.as_deref(), start_time, end_time).await {
            Ok(results) if !results.is_empty() => Ok(results),
            Ok(_) => {
                debug!("No results from InfluxDB, querying SQLite");
                self.sqlite.query_latencies(validator_pubkey, start_time, end_time).await
            }
            Err(e) => {
                warn!("InfluxDB query failed, falling back to SQLite: {}", e);
                self.sqlite.query_latencies(validator_pubkey, start_time, end_time).await
            }
        }
    }
    
    async fn get_validator_info(
        &self,
        pubkey: &solana_sdk::pubkey::Pubkey,
    ) -> crate::error::Result<Option<ValidatorInfo>> {
        // Validator info is only in SQLite
        self.sqlite.get_validator_info(pubkey).await
    }
    
    async fn store_validator_info(&self, info: &ValidatorInfo) -> crate::error::Result<()> {
        // Store validator info only in SQLite
        self.sqlite.store_validator_info(info).await
    }
}

/// Migration status tracker
pub struct MigrationStatus {
    pub total_records: u64,
    pub migrated_records: u64,
    pub failed_records: u64,
    pub start_time: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

impl MigrationStatus {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            total_records: 0,
            migrated_records: 0,
            failed_records: 0,
            start_time: now,
            last_update: now,
        }
    }
    
    pub fn progress_percentage(&self) -> f64 {
        if self.total_records == 0 {
            0.0
        } else {
            (self.migrated_records as f64 / self.total_records as f64) * 100.0
        }
    }
    
    pub fn estimated_completion(&self) -> Option<DateTime<Utc>> {
        if self.migrated_records == 0 {
            return None;
        }
        
        let elapsed = (self.last_update - self.start_time).num_seconds() as f64;
        let rate = self.migrated_records as f64 / elapsed;
        let remaining = self.total_records - self.migrated_records;
        let seconds_remaining = remaining as f64 / rate;
        
        Some(Utc::now() + chrono::Duration::seconds(seconds_remaining as i64))
    }
}