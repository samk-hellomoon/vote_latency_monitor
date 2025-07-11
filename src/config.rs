//! Configuration module for SVLM
//!
//! This module defines the configuration structure and provides
//! utilities for loading and validating configuration from files.

use anyhow::Result;
use config::{Config as ConfigBuilder, File};
use serde::{Deserialize, Serialize};
use std::path::Path;
use super::security;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Application configuration
    pub app: AppConfig,
    
    /// Solana network configuration
    pub solana: SolanaConfig,
    
    /// gRPC subscription configuration
    pub grpc: GrpcConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Metrics configuration
    pub metrics: MetricsConfig,
    
    /// Validator discovery configuration
    pub discovery: DiscoveryConfig,
    
    /// Latency calculation configuration
    pub latency: LatencyConfig,
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Application name
    pub name: String,
    
    /// Log level
    pub log_level: String,
    
    /// Number of worker threads
    pub worker_threads: Option<usize>,
    
    /// Enable debug mode
    pub debug: bool,
}

/// Solana network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    /// RPC endpoint URL
    pub rpc_endpoint: String,
    
    /// Network (mainnet-beta, testnet, devnet)
    pub network: String,
    
    /// Request timeout in seconds
    pub timeout_secs: u64,
    
    /// Maximum concurrent RPC requests
    pub max_concurrent_requests: usize,
}

/// gRPC subscription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// Optional explicit gRPC endpoint (if not set, derived from RPC endpoint)
    pub endpoint: Option<String>,
    
    /// Optional access token for gRPC authentication
    pub access_token: Option<String>,
    
    /// Maximum number of concurrent subscriptions
    pub max_subscriptions: usize,
    
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    
    /// Reconnection interval in seconds
    pub reconnect_interval_secs: u64,
    
    /// Buffer size for incoming transactions
    pub buffer_size: usize,
    
    /// Enable TLS for gRPC connections
    pub enable_tls: bool,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Database file path
    pub database_path: String,
    
    /// Maximum database connections
    pub max_connections: u32,
    
    /// Enable WAL mode for SQLite
    pub enable_wal: bool,
    
    /// Data retention period in days
    pub retention_days: u32,
    
    /// Batch size for bulk inserts
    pub batch_size: usize,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    
    /// Metrics server bind address
    pub bind_address: String,
    
    /// Metrics server port
    pub port: u16,
    
    /// Metrics collection interval in seconds
    pub collection_interval_secs: u64,
}

/// Validator discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Enable automatic validator discovery
    pub enabled: bool,
    
    /// Discovery refresh interval in seconds
    pub refresh_interval_secs: u64,
    
    /// Minimum stake amount for inclusion (in SOL)
    pub min_stake_sol: f64,
    
    /// Include delinquent validators
    pub include_delinquent: bool,
    
    /// Validator whitelist (empty means all)
    pub whitelist: Vec<String>,
    
    /// Validator blacklist
    pub blacklist: Vec<String>,
}

/// Latency calculation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyConfig {
    /// Moving average window size
    pub window_size: usize,
    
    /// Calculate network-wide statistics
    pub calculate_global_stats: bool,
    
    /// Statistics calculation interval in seconds
    pub stats_interval_secs: u64,
    
    /// Outlier detection threshold (standard deviations)
    pub outlier_threshold: f64,
}

impl Config {
    /// Load configuration from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = ConfigBuilder::builder()
            .add_source(File::from(path.as_ref()))
            .add_source(config::Environment::with_prefix("SVLM").separator("_"))
            .build()?;
        
        let config: Config = config.try_deserialize()?;
        config.validate()?;
        
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate RPC endpoint
        if self.solana.rpc_endpoint.is_empty() {
            return Err(anyhow::anyhow!("RPC endpoint cannot be empty"));
        }
        
        // Validate RPC endpoint URL
        security::validate_url(&self.solana.rpc_endpoint, Some(&["http", "https"]))
            .map_err(|e| anyhow::anyhow!("Invalid RPC endpoint URL: {}", e))?;
        
        // Validate network
        let valid_networks = ["mainnet-beta", "testnet", "devnet"];
        if !valid_networks.contains(&self.solana.network.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid network: {}. Must be one of: {:?}",
                self.solana.network,
                valid_networks
            ));
        }
        
        // Validate metrics bind address
        if self.metrics.enabled {
            if self.metrics.port == 0 {
                return Err(anyhow::anyhow!("Metrics port cannot be 0 when metrics are enabled"));
            }
            
            // Warn if binding to all interfaces in production
            if self.metrics.bind_address == "0.0.0.0" && !self.app.debug {
                tracing::warn!(
                    "Metrics server is binding to all interfaces (0.0.0.0). \
                    Consider binding to 127.0.0.1 for better security."
                );
            }
        }
        
        // Validate window size
        if self.latency.window_size == 0 {
            return Err(anyhow::anyhow!("Latency window size must be greater than 0"));
        }
        
        // Validate database path
        if self.storage.database_path.is_empty() {
            return Err(anyhow::anyhow!("Database path cannot be empty"));
        }
        
        // Validate database path against path traversal (allow in-memory for tests)
        if self.storage.database_path != ":memory:" {
            security::validate_path(&self.storage.database_path, None)
                .map_err(|e| anyhow::anyhow!("Invalid database path: {}", e))?;
        }
        
        // Validate retention days
        if self.storage.retention_days == 0 {
            return Err(anyhow::anyhow!("Retention days must be greater than 0"));
        }
        
        // Validate batch size
        if self.storage.batch_size == 0 {
            return Err(anyhow::anyhow!("Batch size must be greater than 0"));
        }
        
        // Validate gRPC buffer size
        if self.grpc.buffer_size == 0 {
            return Err(anyhow::anyhow!("gRPC buffer size must be greater than 0"));
        }
        
        // Validate gRPC endpoint if provided
        if let Some(endpoint) = &self.grpc.endpoint {
            security::validate_url(endpoint, Some(&["http", "https"]))
                .map_err(|e| anyhow::anyhow!("Invalid gRPC endpoint URL: {}", e))?;
        }
        
        // Validate discovery whitelist/blacklist pubkeys
        for pubkey in &self.discovery.whitelist {
            security::validate_pubkey(pubkey)
                .map_err(|e| anyhow::anyhow!("Invalid pubkey in whitelist: {}", e))?;
        }
        
        for pubkey in &self.discovery.blacklist {
            security::validate_pubkey(pubkey)
                .map_err(|e| anyhow::anyhow!("Invalid pubkey in blacklist: {}", e))?;
        }
        
        Ok(())
    }

    /// Create a default configuration for testing
    #[cfg(test)]
    pub fn test_config() -> Self {
        Self::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                name: "svlm".to_string(),
                log_level: "info".to_string(),
                worker_threads: None,
                debug: false,
            },
            solana: SolanaConfig {
                rpc_endpoint: "https://api.mainnet-beta.solana.com".to_string(),
                network: "mainnet-beta".to_string(),
                timeout_secs: 30,
                max_concurrent_requests: 10,
            },
            grpc: GrpcConfig {
                endpoint: None,
                access_token: None,
                max_subscriptions: 100,
                connection_timeout_secs: 30,
                reconnect_interval_secs: 5,
                buffer_size: 10000,
                enable_tls: true,
            },
            storage: StorageConfig {
                database_path: "./data/svlm.db".to_string(),
                max_connections: 10,
                enable_wal: true,
                retention_days: 30,
                batch_size: 1000,
            },
            metrics: MetricsConfig {
                enabled: true,
                bind_address: "127.0.0.1".to_string(),
                port: 9090,
                collection_interval_secs: 60,
            },
            discovery: DiscoveryConfig {
                enabled: true,
                refresh_interval_secs: 300,
                min_stake_sol: 1000.0,
                include_delinquent: false,
                whitelist: vec![],
                blacklist: vec![],
            },
            latency: LatencyConfig {
                window_size: 1000,
                calculate_global_stats: true,
                stats_interval_secs: 60,
                outlier_threshold: 3.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.app.name, "svlm");
        assert_eq!(config.solana.network, "mainnet-beta");
        assert!(config.metrics.enabled);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Empty RPC endpoint should fail
        config.solana.rpc_endpoint = String::new();
        assert!(config.validate().is_err());
        
        // Invalid network should fail
        config = Config::default();
        config.solana.network = "invalid".to_string();
        assert!(config.validate().is_err());
        
        // Zero window size should fail
        config = Config::default();
        config.latency.window_size = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_config_validation_metrics_port() {
        let mut config = Config::default();
        
        // Valid port with metrics enabled
        config.metrics.enabled = true;
        config.metrics.port = 8080;
        assert!(config.validate().is_ok());
        
        // Zero port with metrics enabled should fail
        config.metrics.port = 0;
        assert!(config.validate().is_err());
        
        // Zero port with metrics disabled should pass
        config.metrics.enabled = false;
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_config_validation_database() {
        let mut config = Config::default();
        
        // Empty database path should fail
        config.storage.database_path = String::new();
        assert!(config.validate().is_err());
        
        // Zero retention days should fail
        config = Config::default();
        config.storage.retention_days = 0;
        assert!(config.validate().is_err());
        
        // Zero batch size should fail
        config = Config::default();
        config.storage.batch_size = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_config_validation_grpc() {
        let mut config = Config::default();
        
        // Zero buffer size should fail
        config.grpc.buffer_size = 0;
        assert!(config.validate().is_err());
        
        // Valid buffer size should pass
        config.grpc.buffer_size = 1000;
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_app_config_defaults() {
        let config = Config::default();
        assert_eq!(config.app.name, "svlm");
        assert_eq!(config.app.log_level, "info");
        assert!(!config.app.debug);
        assert!(config.app.worker_threads.is_none());
    }
    
    #[test]
    fn test_solana_config_defaults() {
        let config = Config::default();
        assert_eq!(config.solana.network, "mainnet-beta");
        assert_eq!(config.solana.timeout_secs, 30);
        assert_eq!(config.solana.max_concurrent_requests, 10);
    }
    
    #[test]
    fn test_grpc_config_defaults() {
        let config = Config::default();
        assert_eq!(config.grpc.max_subscriptions, 100);
        assert_eq!(config.grpc.connection_timeout_secs, 30);
        assert_eq!(config.grpc.reconnect_interval_secs, 5);
        assert_eq!(config.grpc.buffer_size, 10000);
        assert!(config.grpc.enable_tls);
    }
    
    #[test]
    fn test_storage_config_defaults() {
        let config = Config::default();
        assert_eq!(config.storage.database_path, "./data/svlm.db");
        assert_eq!(config.storage.max_connections, 10);
        assert!(config.storage.enable_wal);
        assert_eq!(config.storage.retention_days, 30);
        assert_eq!(config.storage.batch_size, 1000);
    }
    
    #[test]
    fn test_discovery_config_defaults() {
        let config = Config::default();
        assert!(config.discovery.enabled);
        assert_eq!(config.discovery.refresh_interval_secs, 300);
        assert_eq!(config.discovery.min_stake_sol, 1000.0);
        assert!(!config.discovery.include_delinquent);
        assert!(config.discovery.whitelist.is_empty());
        assert!(config.discovery.blacklist.is_empty());
    }
    
    #[test]
    fn test_latency_config_defaults() {
        let config = Config::default();
        assert_eq!(config.latency.window_size, 1000);
        assert!(config.latency.calculate_global_stats);
        assert_eq!(config.latency.stats_interval_secs, 60);
        assert_eq!(config.latency.outlier_threshold, 3.0);
    }
}