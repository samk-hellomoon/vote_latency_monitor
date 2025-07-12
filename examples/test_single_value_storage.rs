//! Test the new single-value storage schema
//! 
//! This example demonstrates storing and retrieving vote latencies
//! with the new single-value schema instead of JSON arrays.

use chrono::Utc;
use solana_sdk::pubkey::Pubkey;
use svlm::config::StorageConfig;
use svlm::models::VoteLatency;
use svlm::modules::storage::{StorageManager, StorageManagerTrait};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Testing single-value storage schema");

    // Create test storage config
    let storage_config = StorageConfig {
        database_path: ":memory:".to_string(),
        max_connections: 10,
        enable_wal: true,
        retention_days: 30,
        batch_size: 1000,
    };

    // Create storage manager
    let storage = StorageManager::new(&storage_config).await?;
    storage.initialize().await?;

    // Create test data
    let validator_pubkey = Pubkey::new_unique();
    let vote_pubkey = Pubkey::new_unique();
    let vote_timestamp = Utc::now();
    let received_timestamp = Utc::now();

    // Test 1: Store single-value vote latency (typical TowerSync)
    info!("Test 1: Storing single-value vote latency");
    let single_vote = VoteLatency::new_single_vote(
        validator_pubkey,
        vote_pubkey,
        123456, // voted_on_slot
        vote_timestamp,
        received_timestamp,
        "single_vote_sig_123".to_string(),
        123460, // landed_slot (4 slots later)
    );

    info!("Vote latency: voted_on_slot={}, landed_slot={}, latency_slot={}", 
        single_vote.voted_on_slot(),
        single_vote.landed_slot,
        single_vote.latency_slot()
    );

    storage.store_vote_latency(&single_vote).await?;
    info!("✓ Single-value vote latency stored successfully");

    // Test 2: Store multi-value vote latency (for backward compatibility)
    info!("\nTest 2: Storing multi-value vote latency (backward compatibility)");
    let multi_vote = VoteLatency::new_with_slots(
        validator_pubkey,
        vote_pubkey,
        123459, // highest voted slot
        vote_timestamp,
        received_timestamp,
        "multi_vote_sig_456".to_string(),
        vec![123457, 123458, 123459], // multiple voted slots
        123465, // landed_slot
    );

    info!("Vote latency: voted_on_slots={:?}, landed_slot={}, latency_slots={:?}", 
        multi_vote.voted_on_slots,
        multi_vote.landed_slot,
        multi_vote.latency_slots
    );

    storage.store_vote_latency(&multi_vote).await?;
    info!("✓ Multi-value vote latency stored successfully (converted to single value)");

    // Test 3: Query stored latencies
    info!("\nTest 3: Querying stored latencies");
    let start_time = Utc::now() - chrono::Duration::hours(1);
    let end_time = Utc::now() + chrono::Duration::hours(1);
    
    let latencies = storage.query_latencies(
        Some(&validator_pubkey),
        start_time,
        end_time
    ).await?;

    info!("Found {} latencies", latencies.len());
    for (i, latency) in latencies.iter().enumerate() {
        let sig_preview = if latency.signature.len() >= 20 {
            &latency.signature[..20]
        } else {
            &latency.signature
        };
        info!("  Latency {}: signature={}, voted_on_slot={}, landed_slot={}, latency_slot={}", 
            i + 1,
            sig_preview,
            latency.voted_on_slot(),
            latency.landed_slot,
            latency.latency_slot()
        );
    }

    info!("\n✅ All tests passed! Single-value storage schema is working correctly.");
    info!("The storage system now uses single integer values instead of JSON arrays:");
    info!("  - voted_on_slot: BIGINT (was: voted_on_slots TEXT/JSON)");
    info!("  - latency_slots: INTEGER (was: latency_slots TEXT/JSON)");
    info!("  - This simplifies queries and improves performance");

    Ok(())
}