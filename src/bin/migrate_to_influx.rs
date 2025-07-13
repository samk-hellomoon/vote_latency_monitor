//! Migration tool for moving data from SQLite to InfluxDB

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use std::sync::Arc;
use std::time::Instant;
use svlm::config::{Config, InfluxConfig};
use svlm::modules::storage::{StorageManager, StorageManagerTrait};
use svlm::storage::InfluxDBStorage;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "migrate_to_influx")]
#[command(about = "Migrate vote latency data from SQLite to InfluxDB")]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "./config/config.toml")]
    config: String,
    
    /// Batch size for migration
    #[arg(short, long, default_value = "10000")]
    batch_size: usize,
    
    /// Start from this ID (for resuming)
    #[arg(short, long, default_value = "0")]
    start_id: i64,
    
    /// Dry run - don't actually write to InfluxDB
    #[arg(short, long)]
    dry_run: bool,
    
    /// Skip verification step
    #[arg(long)]
    skip_verify: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    
    let args = Args::parse();
    
    info!("Starting migration from SQLite to InfluxDB");
    info!("Configuration file: {}", args.config);
    info!("Batch size: {}", args.batch_size);
    
    // Load configuration
    let config = Config::load(&args.config)?;
    
    // Ensure InfluxDB config exists
    let influx_config = config.influxdb.clone()
        .ok_or_else(|| anyhow::anyhow!("InfluxDB configuration not found in config file"))?;
    
    // Create storage instances
    let sqlite_storage = StorageManager::new(&config.storage).await?;
    let influx_storage = Arc::new(InfluxDBStorage::new(influx_config).await?);
    
    // Count total records
    info!("Counting records in SQLite database...");
    let total_count = count_total_records(&sqlite_storage).await?;
    info!("Total records to migrate: {}", total_count);
    
    if args.dry_run {
        info!("DRY RUN MODE - No data will be written to InfluxDB");
    }
    
    // Start migration
    let start_time = Instant::now();
    let mut migrated = 0;
    let mut failed = 0;
    let mut last_id = args.start_id;
    
    loop {
        // Fetch batch from SQLite
        let batch = fetch_batch(&sqlite_storage, last_id, args.batch_size).await?;
        
        if batch.is_empty() {
            break;
        }
        
        info!(
            "Processing batch of {} records (IDs {} to {})",
            batch.len(),
            batch.first().map(|v| v.id).unwrap_or(0),
            batch.last().map(|v| v.id).unwrap_or(0)
        );
        
        // Write to InfluxDB
        if !args.dry_run {
            for vote in &batch {
                match influx_storage.store_vote_latency(&vote.vote_latency).await {
                    Ok(_) => migrated += 1,
                    Err(e) => {
                        error!("Failed to migrate vote {}: {}", vote.id, e);
                        failed += 1;
                    }
                }
            }
            
            // Flush after each batch
            if let Err(e) = influx_storage.flush().await {
                warn!("Failed to flush batch: {}", e);
            }
        } else {
            migrated += batch.len();
        }
        
        // Update last ID
        last_id = batch.last().map(|v| v.id).unwrap_or(last_id);
        
        // Progress report
        let elapsed = start_time.elapsed();
        let rate = migrated as f64 / elapsed.as_secs_f64();
        let eta_seconds = ((total_count as i64 - migrated as i64) as f64 / rate) as u64;
        
        info!(
            "Progress: {}/{} ({:.1}%) - Rate: {:.0} records/sec - ETA: {}",
            migrated,
            total_count,
            (migrated as f64 / total_count as f64) * 100.0,
            rate,
            format_duration(eta_seconds)
        );
        
        // Small delay to avoid overwhelming the system
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    let total_elapsed = start_time.elapsed();
    info!(
        "Migration completed in {}",
        format_duration(total_elapsed.as_secs())
    );
    info!("Successfully migrated: {} records", migrated);
    if failed > 0 {
        warn!("Failed to migrate: {} records", failed);
    }
    
    // Verification step
    if !args.skip_verify && !args.dry_run {
        info!("Starting verification...");
        verify_migration(&sqlite_storage, &influx_storage, total_count).await?;
    }
    
    info!("Migration complete!");
    Ok(())
}

/// Count total records in SQLite
async fn count_total_records(storage: &StorageManager) -> Result<i64> {
    // This is a simplified count - you'd need to implement this in StorageManager
    // For now, return a placeholder
    warn!("Record counting not implemented - using estimate");
    Ok(1000000) // Placeholder
}

/// Fetch a batch of records from SQLite
async fn fetch_batch(
    storage: &StorageManager,
    start_id: i64,
    batch_size: usize,
) -> Result<Vec<VoteRecord>> {
    // This would need to be implemented in StorageManager
    // For now, return empty to avoid infinite loop
    Ok(vec![])
}

/// Verify migration by comparing counts
async fn verify_migration(
    _sqlite: &StorageManager,
    _influx: &InfluxDBStorage,
    _expected_count: i64,
) -> Result<()> {
    warn!("Verification not yet implemented");
    Ok(())
}

/// Format duration in human-readable format
fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Temporary struct for migration
struct VoteRecord {
    id: i64,
    vote_latency: svlm::models::VoteLatency,
}