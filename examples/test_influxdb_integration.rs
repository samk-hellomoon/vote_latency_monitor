//! Test InfluxDB integration

use anyhow::Result;
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;
use svlm::config::InfluxConfig;
use svlm::models::{VoteLatency};
use svlm::storage::InfluxDBStorage;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .init();
    
    println!("=== TESTING INFLUXDB INTEGRATION ===\n");
    
    // Create InfluxDB configuration
    let config = InfluxConfig {
        url: "http://localhost:8086".to_string(),
        org: "solana-monitor".to_string(),
        token: "c3oyyJtSYhPP36F8Po4gdh2qgL9A9TP-Q7AWMid7KqLBITDBaog2KBleAFo9AsUD9S9cHwZS10m-8UWAMSi0tA==".to_string(),
        bucket: "vote-latencies-raw".to_string(),
        batch_size: 100,
        flush_interval_ms: 100,
        num_workers: 2,
        enable_compression: true,
    };
    
    // Create storage instance
    println!("Connecting to InfluxDB...");
    let mut storage = InfluxDBStorage::new(config).await?;
    println!("✅ Connected successfully!\n");
    
    // Create test vote latencies
    let validator_pubkey = Pubkey::new_unique();
    let vote_pubkey = Pubkey::new_unique();
    
    println!("Writing test vote latencies...");
    
    // Write multiple vote latencies
    for i in 0..10 {
        let vote_time = Utc::now();
        let latency = VoteLatency::new_single_vote(
            validator_pubkey,
            vote_pubkey,
            1000 + i - 2,  // voted_on_slot
            vote_time,
            vote_time + chrono::Duration::milliseconds(50),
            format!("test_sig_{}", i),
            1000 + i,      // landed_slot
        );
        
        storage.write_vote_latency(&latency).await?;
        println!("  Wrote vote {} with latency {} slots", i, latency.latency_slot());
        
        // Small delay between writes
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Flush any remaining data
    println!("\nFlushing remaining data...");
    storage.flush().await?;
    println!("✅ Flush complete!\n");
    
    // Query back the data
    println!("Querying data back from InfluxDB...");
    let start_time = Utc::now() - chrono::Duration::minutes(5);
    let end_time = Utc::now();
    
    // Note: Query implementation is not complete yet
    match storage.query_latencies(
        Some(&validator_pubkey.to_string()),
        start_time,
        end_time,
    ).await {
        Ok(results) => {
            println!("Found {} vote latencies", results.len());
            for vote in results.iter().take(5) {
                println!("  Vote: slot={}, latency={} slots", 
                    vote.landed_slot, 
                    vote.latency_slot()
                );
            }
        }
        Err(e) => {
            println!("Query not yet implemented: {}", e);
        }
    }
    
    // Test metrics query
    println!("\nTesting metrics query...");
    match storage.get_validator_metrics(
        &validator_pubkey.to_string(),
        std::time::Duration::from_secs(300),
    ).await {
        Ok(metrics) => {
            println!("Metrics retrieved: {:?}", metrics);
        }
        Err(e) => {
            println!("Metrics query not yet implemented: {}", e);
        }
    }
    
    println!("\n✅ InfluxDB integration test complete!");
    println!("\nYou can verify the data in InfluxDB with:");
    println!("influx query 'from(bucket: \"vote-latencies-raw\") |> range(start: -5m) |> filter(fn: (r) => r._measurement == \"vote_latency\")'");
    
    // Give a moment for all async operations to complete
    println!("\nWaiting for all operations to complete...");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Shutdown the storage to clean up background tasks
    println!("Shutting down storage...");
    storage.shutdown().await?;
    println!("✅ Shutdown complete!");
    
    Ok(())
}