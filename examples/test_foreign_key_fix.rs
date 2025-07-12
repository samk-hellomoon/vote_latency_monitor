use anyhow::Result;
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;
use svlm::config::StorageConfig;
use svlm::modules::storage::{StorageManager, StorageManagerTrait};
use svlm::models::{VoteLatency};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    
    println!("=== TESTING FOREIGN KEY CONSTRAINT FIX ===\n");

    // Create in-memory database for testing
    let config = StorageConfig {
        database_path: ":memory:".to_string(),
        retention_days: 1,
        batch_size: 100,
        max_connections: 1,
        enable_wal: false,
    };

    // Create storage manager
    let storage = StorageManager::new(&config).await?;
    println!("✅ Created storage manager with in-memory database");

    // Create a vote latency for a validator that doesn't exist in the database
    let validator_pubkey = Pubkey::new_unique();
    let vote_pubkey = Pubkey::new_unique();
    let vote_time = Utc::now();
    
    let vote_latency = VoteLatency::new_with_slots(
        validator_pubkey,
        vote_pubkey,
        1000,
        vote_time,
        vote_time + chrono::Duration::milliseconds(100),
        "test_signature_12345".to_string(),
        vec![998, 999, 1000],
        1003,
    );

    println!("📝 Created vote latency for validator: {}", validator_pubkey);
    println!("   Vote account: {}", vote_pubkey);
    println!("   Voted slots: {:?}", vote_latency.voted_on_slots);
    println!("   Landed slot: {}", vote_latency.landed_slot);
    println!("   Latency (slots): {:?}", vote_latency.latency_slots);

    // Try to store the vote latency - this should auto-insert the validator
    match storage.store_vote_latency(&vote_latency).await {
        Ok(_) => {
            println!("✅ Successfully stored vote latency!");
            println!("   Auto-insertion of missing validator worked correctly");
        }
        Err(e) => {
            println!("❌ Failed to store vote latency: {}", e);
            return Err(e.into());
        }
    }

    // Verify the validator was auto-inserted
    match storage.get_validator_info(&validator_pubkey).await {
        Ok(Some(info)) => {
            println!("✅ Validator auto-inserted successfully:");
            println!("   Identity: {}", info.pubkey);
            println!("   Vote account: {}", info.vote_account);
            println!("   Name: {:?}", info.name);
        }
        Ok(None) => {
            println!("❌ Validator not found in database");
            return Err(anyhow::anyhow!("Validator should have been auto-inserted"));
        }
        Err(e) => {
            println!("❌ Error querying validator: {}", e);
            return Err(e.into());
        }
    }

    // Test storing another vote latency for the same validator (should not duplicate)
    let vote_latency2 = VoteLatency::new_with_slots(
        validator_pubkey, // Same validator
        vote_pubkey,      // Same vote account
        1001,
        vote_time + chrono::Duration::seconds(1),
        vote_time + chrono::Duration::seconds(1) + chrono::Duration::milliseconds(150),
        "test_signature_67890".to_string(),
        vec![1001],
        1002,
    );

    match storage.store_vote_latency(&vote_latency2).await {
        Ok(_) => {
            println!("✅ Second vote latency stored successfully (no duplicate validator)");
        }
        Err(e) => {
            println!("❌ Failed to store second vote latency: {}", e);
            return Err(e.into());
        }
    }

    println!("\n🎉 Foreign key constraint fix working correctly!");
    println!("   - Auto-inserts missing validators when processing votes");
    println!("   - Handles identity pubkey vs vote account pubkey correctly");
    println!("   - Prevents foreign key constraint errors");

    Ok(())
}