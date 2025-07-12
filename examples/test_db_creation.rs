use anyhow::Result;
use svlm::config::StorageConfig;
use svlm::modules::storage::{StorageManager, StorageManagerTrait};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("=== TESTING DATABASE CREATION ===\n");

    // Test 1: Create database in data directory
    let config = StorageConfig {
        database_path: "./data/test_creation.db".to_string(),
        retention_days: 1,
        batch_size: 100,
        max_connections: 1,
        enable_wal: true,
    };

    println!("Creating database at: {}", config.database_path);
    
    match StorageManager::new(&config).await {
        Ok(_storage) => {
            println!("✅ Successfully created database!");
            
            // Check if file exists
            if std::path::Path::new(&config.database_path).exists() {
                println!("✅ Database file exists at: {}", config.database_path);
            } else {
                println!("❌ Database file was not created!");
            }
        }
        Err(e) => {
            println!("❌ Failed to create database: {}", e);
            return Err(e.into());
        }
    }

    // Test 2: Try with the actual mainnet config path
    println!("\nTesting mainnet database path...");
    let mainnet_config = StorageConfig {
        database_path: "./data/svlm_mainnet.db".to_string(),
        retention_days: 7,
        batch_size: 1000,
        max_connections: 5,
        enable_wal: true,
    };

    match StorageManager::new(&mainnet_config).await {
        Ok(_storage) => {
            println!("✅ Successfully created mainnet database!");
        }
        Err(e) => {
            println!("❌ Failed to create mainnet database: {}", e);
            return Err(e.into());
        }
    }

    // List files in data directory
    println!("\nFiles in data directory:");
    match std::fs::read_dir("./data") {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    println!("  - {}", entry.file_name().to_string_lossy());
                }
            }
        }
        Err(e) => {
            println!("  Error reading directory: {}", e);
        }
    }

    Ok(())
}