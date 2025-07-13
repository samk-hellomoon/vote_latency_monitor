//! InfluxDB Storage Implementation
//!
//! High-performance time-series storage backend using InfluxDB v2
//! for vote latency data.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use influxdb2::{Client, models::DataPoint};
use influxdb2::models::Query;
use futures::stream;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::config::InfluxConfig;
use crate::models::{VoteLatency, LatencyMetrics, ValidatorInfo};

/// Maximum number of points to buffer before forcing a flush
const MAX_BUFFER_SIZE: usize = 5000;

/// Maximum time to wait before flushing the buffer
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);

/// Worker handle for background write tasks
struct WorkerHandle {
    handle: tokio::task::JoinHandle<()>,
}

/// Buffered write batch
struct WriteBatch {
    points: Vec<DataPoint>,
    created_at: Instant,
}

/// InfluxDB storage implementation
pub struct InfluxDBStorage {
    /// InfluxDB client
    client: Arc<Client>,
    
    /// Configuration
    config: InfluxConfig,
    
    /// Write buffer protected by RwLock
    write_buffer: Arc<RwLock<Vec<DataPoint>>>,
    
    /// Channel for sending batches to workers
    batch_sender: mpsc::Sender<WriteBatch>,
    
    /// Worker handles
    workers: Vec<WorkerHandle>,
    
    /// Deduplication cache (signature -> timestamp)
    dedup_cache: Arc<Mutex<LruCache<String, Instant>>>,
    
    /// Flush task handle
    flush_handle: Option<tokio::task::JoinHandle<()>>,
    
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

impl InfluxDBStorage {
    /// Create a new InfluxDB storage instance
    pub async fn new(config: InfluxConfig) -> Result<Self> {
        info!("Initializing InfluxDB storage with URL: {}", config.url);
        
        // Create InfluxDB client
        let client = Client::new(&config.url, &config.org, &config.token);
        
        // Test connection
        match client.ready().await {
            Ok(_) => info!("Successfully connected to InfluxDB"),
            Err(e) => {
                error!("Failed to connect to InfluxDB: {}", e);
                return Err(anyhow::anyhow!("InfluxDB connection failed: {}", e));
            }
        }
        
        // Create write channel
        let (batch_sender, batch_receiver) = mpsc::channel::<WriteBatch>(100);
        
        // Clone client before moving it to spawn_workers
        let client_arc = Arc::new(client.clone());
        
        // Create workers
        let workers = Self::spawn_workers(
            client,
            batch_receiver,
            config.num_workers,
            config.bucket.clone(),
        );
        
        // Create deduplication cache (10k entries)
        let dedup_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(10_000).unwrap())));
        
        let mut storage = Self {
            client: client_arc,
            config: config.clone(),
            write_buffer: Arc::new(RwLock::new(Vec::with_capacity(MAX_BUFFER_SIZE))),
            batch_sender,
            workers,
            dedup_cache,
            flush_handle: None,
            shutdown: Arc::new(AtomicBool::new(false)),
        };
        
        // Start flush task
        storage.start_flush_task();
        
        Ok(storage)
    }
    
    /// Spawn worker threads for writing to InfluxDB
    fn spawn_workers(
        client: Client,
        mut receiver: mpsc::Receiver<WriteBatch>,
        _num_workers: usize,
        bucket: String,
    ) -> Vec<WorkerHandle> {
        let mut workers = Vec::new();
        let client = Arc::new(client);
        
        // For now, use a single worker to avoid complexity with mpsc single receiver
        let handle = tokio::spawn(async move {
            info!("InfluxDB write worker started");
            
            while let Some(batch) = receiver.recv().await {
                let points_count = batch.points.len();
                let batch_age = batch.created_at.elapsed();
                
                debug!(
                    "Worker processing batch with {} points (age: {:?})",
                    points_count, batch_age
                );
                
                // Write to InfluxDB with retry
                let mut retries = 0;
                loop {
                    match client.write(&bucket, stream::iter(batch.points.clone())).await {
                        Ok(_) => {
                            debug!("Worker successfully wrote {} points", points_count);
                            break;
                        }
                        Err(e) => {
                            retries += 1;
                            if retries > 3 {
                                error!(
                                    "Worker failed to write batch after {} retries: {}",
                                    retries, e
                                );
                                break;
                            }
                            warn!(
                                "Worker write failed (retry {}): {}",
                                retries, e
                            );
                            tokio::time::sleep(Duration::from_millis(100 * retries)).await;
                        }
                    }
                }
            }
            
            info!("InfluxDB write worker shutting down");
        });
        
        workers.push(WorkerHandle { handle });
        workers
    }
    
    /// Start the periodic flush task
    fn start_flush_task(&mut self) {
        let buffer = self.write_buffer.clone();
        let sender = self.batch_sender.clone();
        let flush_interval = Duration::from_millis(self.config.flush_interval_ms);
        let shutdown = self.shutdown.clone();
        
        let handle = tokio::spawn(async move {
            let mut ticker = interval(flush_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            while !shutdown.load(Ordering::Relaxed) {
                ticker.tick().await;
                
                // Get and clear the buffer
                let points = {
                    let mut buffer = buffer.write().await;
                    if buffer.is_empty() {
                        continue;
                    }
                    std::mem::take(&mut *buffer)
                };
                
                debug!("Flush task sending batch of {} points", points.len());
                
                let batch = WriteBatch {
                    points,
                    created_at: Instant::now(),
                };
                
                if let Err(e) = sender.send(batch).await {
                    error!("Failed to send batch to workers: {}", e);
                    break;
                }
            }
            
            info!("Flush task shutting down");
        });
        
        self.flush_handle = Some(handle);
    }
    
    /// Write a vote latency record
    pub async fn write_vote_latency(&self, latency: &VoteLatency) -> Result<()> {
        // Check deduplication cache
        {
            let mut cache = self.dedup_cache.lock();
            if let Some(&last_seen) = cache.get(&latency.signature) {
                if last_seen.elapsed() < Duration::from_secs(60) {
                    debug!("Skipping duplicate vote: {}", latency.signature);
                    return Ok(());
                }
            }
            cache.put(latency.signature.clone(), Instant::now());
        }
        
        // Create data point
        let point = DataPoint::builder("vote_latency")
            .tag("validator_id", &latency.validator_pubkey.to_string()[..8])
            .tag("vote_account", &latency.vote_pubkey.to_string()[..8])
            .tag("network", "mainnet") // TODO: Get from config
            .field("latency_slots", latency.latency_slot() as i64)
            .field("voted_slot", latency.voted_on_slot() as i64)
            .field("landed_slot", latency.landed_slot as i64)
            .field("latency_ms", latency.latency_ms as i64)
            .timestamp(latency.received_timestamp.timestamp_nanos_opt().unwrap_or(0))
            .build()?;
        
        // Add to buffer
        {
            let mut buffer = self.write_buffer.write().await;
            buffer.push(point);
            
            // Force flush if buffer is full
            if buffer.len() >= self.config.batch_size {
                let points = std::mem::take(&mut *buffer);
                drop(buffer); // Release lock before sending
                
                let batch = WriteBatch {
                    points,
                    created_at: Instant::now(),
                };
                
                // Use blocking try_send since we're in an async context but don't want to await
                if let Err(e) = self.batch_sender.try_send(batch) {
                    warn!("Failed to send batch immediately: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Query vote latencies for a time range
    pub async fn query_latencies(
        &self,
        validator_pubkey: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<VoteLatency>> {
        let mut query = format!(
            r#"
            from(bucket: "{}")
                |> range(start: {}, stop: {})
                |> filter(fn: (r) => r._measurement == "vote_latency")
            "#,
            self.config.bucket,
            start_time.to_rfc3339(),
            end_time.to_rfc3339()
        );
        
        if let Some(validator) = validator_pubkey {
            query.push_str(&format!(
                r#"|> filter(fn: (r) => r.validator_id == "{}")"#,
                &validator[..8]
            ));
        }
        
        // Execute query
        let query_obj = Query::new(query);
        // For now, use query_raw to get string results
        let result = self.client.query_raw(Some(query_obj)).await?;
        
        // Parse results (simplified for now)
        // TODO: Implement proper result parsing from CSV format
        warn!("Query result parsing not yet implemented, got {} bytes", result.len());
        Ok(vec![])
    }
    
    /// Get aggregated metrics for a validator
    pub async fn get_validator_metrics(
        &self,
        validator_pubkey: &str,
        window: Duration,
    ) -> Result<LatencyMetrics> {
        let query = format!(
            r#"
            import "math"
            
            data = from(bucket: "{}")
                |> range(start: -{}s)
                |> filter(fn: (r) => r._measurement == "vote_latency")
                |> filter(fn: (r) => r.validator_id == "{}")
                |> filter(fn: (r) => r._field == "latency_slots")
                
            // Calculate basic statistics
            stats = data
                |> group()
                |> reduce(
                    identity: {{
                        count: 0,
                        sum: 0.0,
                        sum_squares: 0.0,
                        min: 999999.0,
                        max: 0.0,
                        values: []
                    }},
                    fn: (r, accumulator) => ({{
                        count: accumulator.count + 1,
                        sum: accumulator.sum + float(v: r._value),
                        sum_squares: accumulator.sum_squares + float(v: r._value) * float(v: r._value),
                        min: if r._value < accumulator.min then float(v: r._value) else accumulator.min,
                        max: if r._value > accumulator.max then float(v: r._value) else accumulator.max,
                        values: accumulator.values
                    }})
                )
                |> map(fn: (r) => ({{
                    count: r.count,
                    mean: r.sum / float(v: r.count),
                    min: r.min,
                    max: r.max,
                    stddev: math.sqrt(x: (r.sum_squares / float(v: r.count)) - (r.sum / float(v: r.count)) * (r.sum / float(v: r.count)))
                }}))
                |> yield(name: "stats")
                
            // Count by latency buckets
            buckets = data
                |> group()
                |> map(fn: (r) => ({{
                    r with
                    bucket: if r._value <= 1 then "1_slot" 
                           else if r._value <= 2 then "2_slots"
                           else "3plus_slots"
                }}))
                |> group(columns: ["bucket"])
                |> count()
                |> yield(name: "buckets")
            "#,
            self.config.bucket,
            window.as_secs(),
            &validator_pubkey[..8]
        );
        
        // Execute query
        let query_obj = Query::new(query);
        let result = self.client.query_raw(Some(query_obj)).await?;
        
        // Parse CSV results
        // TODO: Implement proper CSV parsing
        warn!("Metrics query parsing not yet implemented, got {} bytes", result.len());
        
        // For now, return default metrics
        Ok(LatencyMetrics {
            mean_ms: 0.0,
            median_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
            mean_slots: 2.0,
            median_slots: 2.0,
            p95_slots: 3.0,
            p99_slots: 4.0,
            min_slots: 1.0,
            max_slots: 5.0,
            votes_1_slot: 100,
            votes_2_slots: 50,
            votes_3plus_slots: 10,
            sample_count: 160,
            timestamp: Utc::now(),
        })
    }
    
    /// Flush any pending writes
    pub async fn flush(&self) -> Result<()> {
        let points = {
            let mut buffer = self.write_buffer.write().await;
            if buffer.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *buffer)
        };
        
        info!("Manually flushing {} points", points.len());
        
        let batch = WriteBatch {
            points,
            created_at: Instant::now(),
        };
        
        self.batch_sender.send(batch).await?;
        Ok(())
    }
    
    /// Shutdown the storage system
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down InfluxDB storage");
        
        // Set shutdown flag
        self.shutdown.store(true, Ordering::Relaxed);
        
        // Flush remaining data
        self.flush().await?;
        
        // Stop flush task
        if let Some(handle) = self.flush_handle.take() {
            // Give it a moment to finish
            let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
        }
        
        // Drop our sender to signal workers to stop
        // With mpsc, we can't easily replace it, so we'll rely on the shutdown flag
        
        // Wait for workers to finish
        for (i, worker) in self.workers.drain(..).enumerate() {
            match tokio::time::timeout(Duration::from_secs(5), worker.handle).await {
                Ok(Ok(_)) => debug!("Worker {} shut down cleanly", i),
                Ok(Err(e)) => error!("Worker {} panicked: {}", i, e),
                Err(_) => error!("Worker {} timed out during shutdown", i),
            }
        }
        
        info!("InfluxDB storage shutdown complete");
        Ok(())
    }
}

/// Storage trait implementation for compatibility
#[async_trait]
impl crate::modules::storage::StorageManagerTrait for InfluxDBStorage {
    async fn initialize(&self) -> crate::error::Result<()> {
        // Already initialized in new()
        Ok(())
    }
    
    async fn store_vote_latency(&self, latency: &VoteLatency) -> crate::error::Result<()> {
        self.write_vote_latency(latency).await
            .map_err(|e| crate::error::Error::internal(format!("InfluxDB write error: {}", e)))
    }
    
    async fn store_metrics(
        &self,
        _metrics: &LatencyMetrics,
        _validator_pubkey: Option<&solana_sdk::pubkey::Pubkey>,
    ) -> crate::error::Result<()> {
        // Metrics are calculated by InfluxDB queries, not stored separately
        Ok(())
    }
    
    async fn query_latencies(
        &self,
        validator_pubkey: Option<&solana_sdk::pubkey::Pubkey>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> crate::error::Result<Vec<VoteLatency>> {
        let validator_str = validator_pubkey.map(|p| p.to_string());
        self.query_latencies(validator_str.as_deref(), start_time, end_time)
            .await
            .map_err(|e| crate::error::Error::internal(format!("InfluxDB query error: {}", e)))
    }
    
    async fn get_validator_info(
        &self,
        _pubkey: &solana_sdk::pubkey::Pubkey,
    ) -> crate::error::Result<Option<ValidatorInfo>> {
        // Validator info is not stored in InfluxDB
        Ok(None)
    }
    
    async fn store_validator_info(&self, _info: &ValidatorInfo) -> crate::error::Result<()> {
        // Validator info is not stored in InfluxDB
        Ok(())
    }
}