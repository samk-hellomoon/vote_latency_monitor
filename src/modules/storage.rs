//! Storage Manager Module
//!
//! This module handles persistent storage of vote latency data and metrics.
//! It uses SQLite for local storage and provides interfaces for querying
//! historical data and generating reports.

use anyhow::Result as AnyResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_sdk::pubkey::Pubkey;
use sqlx::{sqlite::SqlitePool, Row};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

use crate::config::StorageConfig;
use crate::error::{Error, Result};
use crate::models::{LatencyMetrics, ValidatorInfo, VoteLatency};
use crate::modules::Shutdown;
use crate::security;

/// Trait for storage implementations
#[async_trait]
pub trait StorageManagerTrait: Send + Sync {
    /// Initialize the database
    async fn initialize(&self) -> Result<()>;
    
    /// Store a vote latency record
    async fn store_vote_latency(&self, latency: &VoteLatency) -> Result<()>;
    
    /// Store aggregated metrics
    async fn store_metrics(&self, metrics: &LatencyMetrics, validator_pubkey: Option<&Pubkey>) -> Result<()>;
    
    /// Query vote latencies for a time range
    async fn query_latencies(
        &self,
        validator_pubkey: Option<&Pubkey>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<VoteLatency>>;
    
    /// Get validator information
    async fn get_validator_info(&self, pubkey: &Pubkey) -> Result<Option<ValidatorInfo>>;
    
    /// Store validator information
    async fn store_validator_info(&self, info: &ValidatorInfo) -> Result<()>;
}

/// SQLite-based storage manager
pub struct StorageManager {
    pool: Arc<SqlitePool>,
    config: StorageConfig,
}

impl StorageManager {
    /// Create a new storage manager
    pub async fn new(config: &StorageConfig) -> Result<Self> {
        // Validate and normalize database path
        let db_path = if config.database_path == ":memory:" {
            // Special case for in-memory database
            std::path::PathBuf::from(":memory:")
        } else {
            // Validate path to prevent traversal attacks
            security::validate_path(&config.database_path, None)?
        };
        
        // Ensure parent directory exists (except for in-memory)
        if config.database_path != ":memory:" {
            if let Some(parent) = db_path.parent() {
                info!("Creating database directory: {}", parent.display());
                std::fs::create_dir_all(parent)?;
            }
            
            // Log if database file doesn't exist yet
            if !db_path.exists() {
                info!("Database file does not exist, will be created: {}", db_path.display());
            }
        }
        
        // Create database URL with create mode
        let database_url = if config.database_path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            // Use create mode to ensure database is created if it doesn't exist
            format!("sqlite://{}?mode=rwc", db_path.display())
        };
        
        debug!("Connecting to database: {}", database_url);
        let pool = SqlitePool::connect(&database_url).await?;
        
        let manager = Self {
            pool: Arc::new(pool),
            config: config.clone(),
        };
        
        // Initialize database
        manager.initialize().await?;
        
        Ok(manager)
    }
    
    /// Create a new storage (compatibility alias)
    pub async fn new_from_config(config: &StorageConfig) -> Result<Arc<dyn StorageManagerTrait>> {
        Ok(Arc::new(Self::new(config).await?) as Arc<dyn StorageManagerTrait>)
    }

    /// Apply SQLite performance optimizations
    async fn apply_sqlite_optimizations(&self) -> Result<()> {
        // Apply PRAGMAs for optimal performance as specified in architecture doc
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&*self.pool)
            .await?;
        
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&*self.pool)
            .await?;
        
        sqlx::query("PRAGMA cache_size = -64000")  // 64MB cache
            .execute(&*self.pool)
            .await?;
        
        sqlx::query("PRAGMA page_size = 4096")     // Optimal for SSDs
            .execute(&*self.pool)
            .await?;
        
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&*self.pool)
            .await?;
        
        sqlx::query("PRAGMA mmap_size = 30000000000")  // 30GB mmap
            .execute(&*self.pool)
            .await?;
        
        info!("Applied SQLite performance optimizations");
        Ok(())
    }

    /// Create database tables
    async fn create_tables(&self) -> Result<()> {
        // Validators table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS validators (
                pubkey TEXT PRIMARY KEY,
                vote_account TEXT NOT NULL,
                name TEXT,
                description TEXT,
                website TEXT,
                grpc_endpoint TEXT,
                discovered_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&*self.pool)
        .await?;

        // Vote latencies table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS vote_latencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                validator_pubkey TEXT NOT NULL,
                vote_pubkey TEXT NOT NULL,
                slot BIGINT NOT NULL,
                signature TEXT NOT NULL UNIQUE,
                vote_timestamp TIMESTAMP NOT NULL,
                received_timestamp TIMESTAMP NOT NULL,
                latency_ms INTEGER NOT NULL,
                voted_on_slot BIGINT,  -- Single slot that was voted on
                landed_slot BIGINT,
                latency_slots INTEGER,  -- Single latency value in slots
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (validator_pubkey) REFERENCES validators(pubkey)
            )
            "#,
        )
        .execute(&*self.pool)
        .await?;

        // Metrics table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                validator_pubkey TEXT,
                mean_ms REAL NOT NULL,
                median_ms REAL NOT NULL,
                p95_ms REAL NOT NULL,
                p99_ms REAL NOT NULL,
                min_ms REAL NOT NULL,
                max_ms REAL NOT NULL,
                mean_slots REAL,
                median_slots REAL,
                p95_slots REAL,
                p99_slots REAL,
                min_slots REAL,
                max_slots REAL,
                votes_1_slot INTEGER,
                votes_2_slots INTEGER,
                votes_3plus_slots INTEGER,
                sample_count INTEGER NOT NULL,
                timestamp TIMESTAMP NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (validator_pubkey) REFERENCES validators(pubkey)
            )
            "#,
        )
        .execute(&*self.pool)
        .await?;

        // Create indexes
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_vote_latencies_validator_timestamp 
            ON vote_latencies(validator_pubkey, vote_timestamp);
            
            CREATE INDEX IF NOT EXISTS idx_vote_latencies_slot 
            ON vote_latencies(slot);
            
            CREATE INDEX IF NOT EXISTS idx_vote_latencies_landed_slot 
            ON vote_latencies(landed_slot);
            
            CREATE INDEX IF NOT EXISTS idx_metrics_validator_timestamp 
            ON metrics(validator_pubkey, timestamp);
            "#,
        )
        .execute(&*self.pool)
        .await?;

        info!("Database tables created successfully");
        Ok(())
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // Apply optimizations first (only for non-memory databases)
        if self.config.database_path != ":memory:" {
            self.apply_sqlite_optimizations().await?;
        }
        
        // Create tables
        self.create_tables().await?;
        
        // Check if we need to add slot columns (for existing databases)
        self.migrate_add_slot_columns().await?;
        
        info!("Database setup completed successfully");
        Ok(())
    }
    
    /// Add slot-based columns to existing tables if they don't exist
    async fn migrate_add_slot_columns(&self) -> Result<()> {
        // Check if voted_on_slot column exists (new single-value column)
        let column_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('vote_latencies') WHERE name = 'voted_on_slot'"
        )
        .fetch_one(&*self.pool)
        .await?;
        
        if !column_exists {
            info!("Migrating database to add slot-based columns...");
            
            // Add columns to vote_latencies
            sqlx::query("ALTER TABLE vote_latencies ADD COLUMN voted_on_slot BIGINT")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE vote_latencies ADD COLUMN landed_slot BIGINT")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE vote_latencies ADD COLUMN latency_slots INTEGER")
                .execute(&*self.pool)
                .await?;
            
            // Add index
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_vote_latencies_landed_slot ON vote_latencies(landed_slot)")
                .execute(&*self.pool)
                .await?;
            
            // Backfill data
            sqlx::query(
                r#"
                UPDATE vote_latencies 
                SET voted_on_slot = slot,
                    landed_slot = slot,
                    latency_slots = 0
                WHERE voted_on_slot IS NULL
                "#
            )
            .execute(&*self.pool)
            .await?;
            
            info!("Added slot-based columns to vote_latencies table");
        }
        
        // Check if we need to migrate from JSON arrays to single values
        let has_json_columns: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('vote_latencies') WHERE name = 'voted_on_slots'"
        )
        .fetch_one(&*self.pool)
        .await?;
        
        if has_json_columns {
            info!("Migrating from JSON arrays to single values...");
            
            // First, ensure the new columns exist
            sqlx::query("ALTER TABLE vote_latencies ADD COLUMN IF NOT EXISTS voted_on_slot BIGINT")
                .execute(&*self.pool)
                .await
                .ok(); // Ignore error if column already exists
            
            sqlx::query("ALTER TABLE vote_latencies ADD COLUMN IF NOT EXISTS latency_slots INTEGER")
                .execute(&*self.pool)
                .await
                .ok(); // Ignore error if column already exists
            
            // Migrate data from JSON to single values
            // For voted_on_slots, take the last (most recent) value from the JSON array
            // For latency_slots, take the last value as well
            sqlx::query(
                r#"
                UPDATE vote_latencies 
                SET voted_on_slot = CAST(json_extract(voted_on_slots, '$[#-1]') AS BIGINT),
                    latency_slots = CAST(json_extract(latency_slots, '$[#-1]') AS INTEGER)
                WHERE voted_on_slots IS NOT NULL 
                  AND json_valid(voted_on_slots)
                  AND json_valid(latency_slots)
                "#
            )
            .execute(&*self.pool)
            .await?;
            
            info!("Migrated JSON data to single values");
            
            // Note: We don't drop the old columns here to allow for rollback if needed
            // They can be dropped in a later migration after confirming the data is correct
        }
        
        // Check if mean_slots column exists in metrics table
        let metrics_column_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('metrics') WHERE name = 'mean_slots'"
        )
        .fetch_one(&*self.pool)
        .await?;
        
        if !metrics_column_exists {
            // Add columns to metrics
            sqlx::query("ALTER TABLE metrics ADD COLUMN mean_slots REAL")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN median_slots REAL")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN p95_slots REAL")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN p99_slots REAL")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN min_slots REAL")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN max_slots REAL")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN votes_1_slot INTEGER")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN votes_2_slots INTEGER")
                .execute(&*self.pool)
                .await?;
            sqlx::query("ALTER TABLE metrics ADD COLUMN votes_3plus_slots INTEGER")
                .execute(&*self.pool)
                .await?;
            
            info!("Added slot-based columns to metrics table");
        }
        
        Ok(())
    }
}

#[async_trait]
impl StorageManagerTrait for StorageManager {
    async fn initialize(&self) -> Result<()> {
        info!("Initializing storage manager");
        
        // Apply SQLite optimizations first (only for non-memory databases)
        if self.config.database_path != ":memory:" {
            self.apply_sqlite_optimizations().await?;
        }
        
        // Create tables
        self.create_tables().await?;
        
        // Run slot column migration
        self.migrate_add_slot_columns().await?;
        
        // Run migrations if available
        if Path::new("./migrations").exists() {
            info!("Migrations directory found - slot migration already applied inline");
        }
        
        Ok(())
    }

    async fn store_vote_latency(&self, latency: &VoteLatency) -> Result<()> {
        debug!("Storing vote latency for validator: {}", latency.validator_pubkey);
        
        // Validate signature (should be base58 encoded, max 88 chars for ed25519)
        let signature = security::validate_string(&latency.signature, "signature", 128)?;
        if signature.is_empty() {
            return Err(Error::parse("Signature cannot be empty"));
        }
        
        // Validate slot is reasonable (not too far in future)
        if latency.slot > u64::MAX / 2 {
            return Err(Error::parse("Slot number suspiciously large"));
        }
        
        // Auto-insert validator if it doesn't exist to prevent foreign key constraint errors
        self.ensure_validator_exists(latency.validator_pubkey, latency.vote_pubkey).await?;
        
        // Get the single voted slot and latency value
        // For compatibility, use the last (most recent) values if still using arrays
        let voted_on_slot = latency.voted_on_slots.last().copied().unwrap_or(latency.slot);
        let latency_slots_value = latency.latency_slots.last().copied().unwrap_or(0) as i64;
        
        sqlx::query(
            r#"
            INSERT INTO vote_latencies (
                validator_pubkey, vote_pubkey, slot, signature,
                vote_timestamp, received_timestamp, latency_ms,
                voted_on_slot, landed_slot, latency_slots
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(latency.validator_pubkey.to_string())
        .bind(latency.vote_pubkey.to_string())
        .bind(latency.slot as i64)
        .bind(signature)
        .bind(latency.vote_timestamp)
        .bind(latency.received_timestamp)
        .bind(latency.latency_ms as i64)
        .bind(voted_on_slot as i64)
        .bind(latency.landed_slot as i64)
        .bind(latency_slots_value)
        .execute(&*self.pool)
        .await?;
        
        Ok(())
    }

    async fn store_metrics(
        &self,
        metrics: &LatencyMetrics,
        validator_pubkey: Option<&Pubkey>,
    ) -> Result<()> {
        debug!("Storing metrics");
        
        sqlx::query(
            r#"
            INSERT INTO metrics (
                validator_pubkey, mean_ms, median_ms, p95_ms, p99_ms,
                min_ms, max_ms, mean_slots, median_slots, p95_slots,
                p99_slots, min_slots, max_slots, votes_1_slot,
                votes_2_slots, votes_3plus_slots, sample_count, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(validator_pubkey.map(|p| p.to_string()))
        .bind(metrics.mean_ms)
        .bind(metrics.median_ms)
        .bind(metrics.p95_ms)
        .bind(metrics.p99_ms)
        .bind(metrics.min_ms)
        .bind(metrics.max_ms)
        .bind(metrics.mean_slots as f64)
        .bind(metrics.median_slots as f64)
        .bind(metrics.p95_slots as f64)
        .bind(metrics.p99_slots as f64)
        .bind(metrics.min_slots as f64)
        .bind(metrics.max_slots as f64)
        .bind(metrics.votes_1_slot as i64)
        .bind(metrics.votes_2_slots as i64)
        .bind(metrics.votes_3plus_slots as i64)
        .bind(metrics.sample_count as i64)
        .bind(metrics.timestamp)
        .execute(&*self.pool)
        .await?;
        
        Ok(())
    }

    async fn query_latencies(
        &self,
        validator_pubkey: Option<&Pubkey>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<VoteLatency>> {
        let query = if let Some(pubkey) = validator_pubkey {
            sqlx::query(
                r#"
                SELECT validator_pubkey, vote_pubkey, slot, signature,
                       vote_timestamp, received_timestamp, latency_ms,
                       voted_on_slot, landed_slot, latency_slots
                FROM vote_latencies
                WHERE validator_pubkey = ? 
                  AND vote_timestamp >= ? 
                  AND vote_timestamp <= ?
                ORDER BY vote_timestamp DESC
                "#,
            )
            .bind(pubkey.to_string())
            .bind(start_time)
            .bind(end_time)
        } else {
            sqlx::query(
                r#"
                SELECT validator_pubkey, vote_pubkey, slot, signature,
                       vote_timestamp, received_timestamp, latency_ms,
                       voted_on_slot, landed_slot, latency_slots
                FROM vote_latencies
                WHERE vote_timestamp >= ? 
                  AND vote_timestamp <= ?
                ORDER BY vote_timestamp DESC
                "#,
            )
            .bind(start_time)
            .bind(end_time)
        };

        let rows = query.fetch_all(&*self.pool).await?;
        
        let latencies = rows
            .into_iter()
            .map(|row| {
                // Get single values instead of JSON arrays
                let voted_on_slot = row.get::<Option<i64>, _>(7)
                    .map(|s| s as u64)
                    .unwrap_or(row.get::<i64, _>(2) as u64);
                
                let landed_slot = row.get::<Option<i64>, _>(8)
                    .map(|s| s as u64)
                    .unwrap_or(row.get::<i64, _>(2) as u64);
                
                let latency_slots_value = row.get::<Option<i64>, _>(9)
                    .map(|s| s as u8)
                    .unwrap_or(0);
                
                Ok(VoteLatency {
                    validator_pubkey: row.get::<String, _>(0).parse()?,
                    vote_pubkey: row.get::<String, _>(1).parse()?,
                    slot: row.get::<i64, _>(2) as u64,
                    signature: row.get(3),
                    vote_timestamp: row.get(4),
                    received_timestamp: row.get(5),
                    latency_ms: row.get::<i64, _>(6) as u64,
                    voted_on_slots: vec![voted_on_slot], // For compatibility, wrap in vec
                    landed_slot,
                    latency_slots: vec![latency_slots_value], // For compatibility, wrap in vec
                })
            })
            .collect::<AnyResult<Vec<_>>>()?;
        
        Ok(latencies)
    }

    async fn get_validator_info(&self, pubkey: &Pubkey) -> Result<Option<ValidatorInfo>> {
        let row = sqlx::query(
            r#"
            SELECT pubkey, vote_account, name, description, website, grpc_endpoint
            FROM validators
            WHERE pubkey = ?
            "#,
        )
        .bind(pubkey.to_string())
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(ValidatorInfo {
                pubkey: row.get::<String, _>(0).parse()?,
                vote_account: row.get::<String, _>(1).parse()?,
                name: row.get(2),
                description: row.get(3),
                website: row.get(4),
                grpc_endpoint: row.get(5),
            }))
        } else {
            Ok(None)
        }
    }

    async fn store_validator_info(&self, info: &ValidatorInfo) -> Result<()> {
        // Validate pubkeys
        let pubkey_str = info.pubkey.to_string();
        let vote_account_str = info.vote_account.to_string();
        
        // Validate optional fields
        let name = if let Some(name) = &info.name {
            Some(security::validate_string(name, "validator name", security::MAX_STRING_LENGTH)?)
        } else {
            None
        };
        
        let description = if let Some(desc) = &info.description {
            Some(security::validate_string(desc, "validator description", security::MAX_DESCRIPTION_LENGTH)?)
        } else {
            None
        };
        
        let website = if let Some(url) = &info.website {
            Some(security::validate_url(url, Some(&["http", "https"]))?)
        } else {
            None
        };
        
        let grpc_endpoint = if let Some(endpoint) = &info.grpc_endpoint {
            Some(security::validate_url(endpoint, Some(&["http", "https", "grpc", "grpcs"]))?)
        } else {
            None
        };
        
        sqlx::query(
            r#"
            INSERT INTO validators (
                pubkey, vote_account, name, description, website, grpc_endpoint
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(pubkey) DO UPDATE SET
                vote_account = excluded.vote_account,
                name = excluded.name,
                description = excluded.description,
                website = excluded.website,
                grpc_endpoint = excluded.grpc_endpoint,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(pubkey_str)
        .bind(vote_account_str)
        .bind(name)
        .bind(description)
        .bind(website)
        .bind(grpc_endpoint)
        .execute(&*self.pool)
        .await?;
        
        Ok(())
    }
}

impl StorageManager {
    /// Ensure a validator exists in the database, creating it if necessary
    /// This prevents foreign key constraint errors when storing vote latencies.
    /// 
    /// Key insight: Solana validators have TWO pubkeys:
    /// - validator_pubkey (identity): The validator's main identity, stored in 'validators' table
    /// - vote_pubkey (vote account): The account that receives vote transactions
    /// 
    /// Vote transactions reference the validator's identity pubkey, but are processed
    /// via vote account subscriptions. This method ensures the identity pubkey exists
    /// in the validators table when we receive votes for it.
    async fn ensure_validator_exists(&self, validator_pubkey: Pubkey, vote_pubkey: Pubkey) -> Result<()> {
        debug!("Ensuring validator exists: identity={}, vote_account={}", validator_pubkey, vote_pubkey);
        
        // Check if validator already exists
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM validators WHERE pubkey = ?"
        )
        .bind(validator_pubkey.to_string())
        .fetch_one(&*self.pool)
        .await? > 0;
        
        if !exists {
            debug!("Auto-inserting missing validator: {}", validator_pubkey);
            
            // Create a minimal ValidatorInfo and store it
            let validator_info = ValidatorInfo::new(validator_pubkey, vote_pubkey);
            self.store_validator_info(&validator_info).await?;
            
            info!("Auto-inserted validator {} with vote account {} from vote transaction", 
                  validator_pubkey, vote_pubkey);
        }
        
        Ok(())
    }
}

#[async_trait]
impl Shutdown for StorageManager {
    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down storage manager");
        
        // Close the connection pool
        self.pool.close().await;
        
        info!("Storage manager shutdown complete");
        Ok(())
    }
}

/// Storage wrapper for use in main
pub struct Storage {
    inner: Arc<dyn StorageManagerTrait>,
}

impl Storage {
    /// Create new storage instance
    pub async fn new(config: &StorageConfig) -> Result<Self> {
        let inner = StorageManager::new_from_config(config).await?;
        Ok(Self { inner })
    }
    
    /// Run migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // This is a placeholder - actual implementation would depend on the trait
        info!("Running database migrations...");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_storage() -> Result<(StorageManager, TempDir)> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        
        let config = StorageConfig {
            database_path: db_path.to_string_lossy().to_string(),
            max_connections: 10,
            enable_wal: true,
            retention_days: 30,
            batch_size: 1000,
        };
        
        let storage = StorageManager::new(&config).await?;
        
        Ok((storage, temp_dir))
    }

    #[tokio::test]
    async fn test_storage_initialization() {
        let (storage, _temp_dir) = create_test_storage().await.unwrap();
        
        // Verify validators table exists
        let validators_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM validators")
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert_eq!(validators_count, 0);
        
        // Verify vote_latencies table exists
        let latencies_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vote_latencies")
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert_eq!(latencies_count, 0);
        
        // Verify metrics table exists
        let metrics_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM metrics")
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert_eq!(metrics_count, 0);
        
        // Verify indexes were created
        let index_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert!(index_count >= 3); // We expect at least 3 indexes
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_data() {
        let (storage, _temp_dir) = create_test_storage().await.unwrap();
        
        // Create test validator info
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let validator_info = ValidatorInfo {
            pubkey: validator_pubkey,
            vote_account: vote_pubkey,
            name: Some("Test Validator".to_string()),
            description: Some("A test validator".to_string()),
            website: Some("https://example.com".to_string()),
            grpc_endpoint: Some("https://grpc.example.com".to_string()),
        };
        
        // Store validator info
        storage.store_validator_info(&validator_info).await.unwrap();
        
        // Retrieve and verify
        let retrieved = storage.get_validator_info(&validator_pubkey).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.pubkey, validator_pubkey);
        assert_eq!(retrieved.vote_account, vote_pubkey);
        assert_eq!(retrieved.name, Some("Test Validator".to_string()));
        
        // Create and store vote latency with slot-based data
        let vote_latency = VoteLatency {
            validator_pubkey,
            vote_pubkey,
            slot: 12345,
            signature: "test_signature_12345".to_string(),
            vote_timestamp: Utc::now(),
            received_timestamp: Utc::now(),
            latency_ms: 150,
            voted_on_slots: vec![12345], // Single value for new schema
            landed_slot: 12347,
            latency_slots: vec![2], // Single latency value
        };
        
        storage.store_vote_latency(&vote_latency).await.unwrap();
        
        // Query latencies
        let start_time = Utc::now() - chrono::Duration::hours(1);
        let end_time = Utc::now() + chrono::Duration::hours(1);
        let latencies = storage.query_latencies(
            Some(&validator_pubkey),
            start_time,
            end_time
        ).await.unwrap();
        
        assert_eq!(latencies.len(), 1);
        assert_eq!(latencies[0].slot, 12345);
        assert_eq!(latencies[0].latency_ms, 150);
        assert_eq!(latencies[0].voted_on_slots, vec![12345]); // Single value
        assert_eq!(latencies[0].landed_slot, 12347);
        assert_eq!(latencies[0].latency_slots, vec![2]); // Single latency value
        
        // Store metrics
        let metrics = LatencyMetrics {
            mean_ms: 150.0,
            median_ms: 150.0,
            p95_ms: 200.0,
            p99_ms: 250.0,
            min_ms: 100.0,
            max_ms: 300.0,
            mean_slots: 2.5,
            median_slots: 2.0,
            p95_slots: 4.0,
            p99_slots: 5.0,
            min_slots: 1.0,
            max_slots: 5.0,
            votes_1_slot: 20,
            votes_2_slots: 30,
            votes_3plus_slots: 50,
            sample_count: 100,
            timestamp: Utc::now(),
        };
        
        storage.store_metrics(&metrics, Some(&validator_pubkey)).await.unwrap();
        
        // Verify data was stored by checking row counts
        let validator_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM validators")
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert_eq!(validator_count, 1);
        
        let latency_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vote_latencies")
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert_eq!(latency_count, 1);
        
        let metrics_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM metrics")
            .fetch_one(&*storage.pool)
            .await
            .unwrap();
        assert_eq!(metrics_count, 1);
    }
}