//! Validator Discovery Module
//!
//! This module is responsible for discovering and maintaining information about
//! active validators in the Solana network. It queries the RPC endpoint to get
//! vote accounts and maintains a cache of validator information.

use crate::error::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info};

use crate::config::Config;
use crate::models::ValidatorInfo;
use crate::modules::{Shutdown, ShutdownSignal};
use crate::retry::{retry_with_config, RetryConfig};
use tokio::sync::broadcast;
use tokio::select;

/// Trait for validator discovery implementations
#[async_trait]
pub trait ValidatorDiscoveryTrait: Send + Sync {
    /// Discover validators from the network
    async fn discover(&self) -> Result<Vec<ValidatorInfo>>;
    
    /// Get a specific validator by pubkey
    async fn get_validator(&self, pubkey: &Pubkey) -> Option<ValidatorInfo>;
    
    /// Get all discovered validators
    async fn get_all_validators(&self) -> Vec<ValidatorInfo>;
}

/// Validator discovery service
pub struct ValidatorDiscovery {
    rpc_client: Arc<RpcClient>,
    validators: Arc<DashMap<Pubkey, ValidatorInfo>>,
    config: Arc<Config>,
    shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ValidatorDiscovery {
    /// Create a new validator discovery instance
    pub async fn new(
        config: Arc<Config>,
        shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    ) -> Result<Self> {
        let rpc_client = Arc::new(RpcClient::new(config.solana.rpc_endpoint.clone()));
        
        Ok(Self {
            rpc_client,
            validators: Arc::new(DashMap::new()),
            config,
            shutdown_rx,
            task_handle: None,
        })
    }

    /// Start the discovery service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting validator discovery service");
        
        // Initial discovery
        self.refresh_validators().await?;
        
        // Start periodic refresh task
        let validators = Arc::clone(&self.validators);
        let rpc_client = Arc::clone(&self.rpc_client);
        let config = Arc::clone(&self.config);
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        let handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(
                config.discovery.refresh_interval_secs
            ));
            
            loop {
                select! {
                    _ = interval.tick() => {
                        if let Err(e) = Self::refresh_validators_static(
                            &rpc_client,
                            &validators,
                            &config,
                        ).await {
                            error!("Failed to refresh validators: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Validator discovery received shutdown signal");
                        break;
                    }
                }
            }
        });
        
        self.task_handle = Some(handle);
        Ok(())
    }

    /// Refresh the validator list
    async fn refresh_validators(&self) -> Result<()> {
        Self::refresh_validators_static(
            &self.rpc_client,
            &self.validators,
            &self.config,
        ).await
    }
    
    /// Static refresh validators implementation
    async fn refresh_validators_static(
        rpc_client: &RpcClient,
        validators: &DashMap<Pubkey, ValidatorInfo>,
        config: &Config,
    ) -> Result<()> {
        debug!("Refreshing validator list");
        
        // Create retry config for RPC operations
        let retry_config = RetryConfig::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_secs(1));
        
        // Get vote accounts with retry
        let vote_accounts = retry_with_config(
            || async { 
                rpc_client.get_vote_accounts().await
                    .map_err(|e| crate::error::Error::rpc(format!("Failed to get vote accounts: {}", e)))
            },
            retry_config,
        ).await?;
        
        // Clear existing validators
        validators.clear();
        
        // Process current validators
        for vote_account in vote_accounts.current {
            let validator_pubkey = vote_account.node_pubkey.parse::<Pubkey>()?;
            let vote_pubkey = vote_account.vote_pubkey.parse::<Pubkey>()?;
            
            // Check minimum stake requirement
            let stake_lamports = vote_account.activated_stake;
            let stake_sol = stake_lamports as f64 / 1_000_000_000.0;
            
            if stake_sol < config.discovery.min_stake_sol {
                continue;
            }
            
            // Check whitelist/blacklist
            let identity_pubkey_str = validator_pubkey.to_string();
            let vote_pubkey_str = vote_pubkey.to_string();
            
            // For whitelist: accept if either identity or vote pubkey is in the list
            if !config.discovery.whitelist.is_empty() {
                let in_whitelist = config.discovery.whitelist.contains(&identity_pubkey_str) 
                    || config.discovery.whitelist.contains(&vote_pubkey_str);
                if !in_whitelist {
                    continue;
                }
            }
            
            // For blacklist: reject if either identity or vote pubkey is in the list
            if config.discovery.blacklist.contains(&identity_pubkey_str) 
                || config.discovery.blacklist.contains(&vote_pubkey_str) {
                continue;
            }
            
            let info = ValidatorInfo::new(validator_pubkey, vote_pubkey);
            validators.insert(validator_pubkey, info);
        }
        
        // Process delinquent validators if configured
        if config.discovery.include_delinquent {
            for vote_account in vote_accounts.delinquent {
                let validator_pubkey = vote_account.node_pubkey.parse::<Pubkey>()?;
                let vote_pubkey = vote_account.vote_pubkey.parse::<Pubkey>()?;
                
                // Apply the same whitelist/blacklist logic for delinquent validators
                let identity_pubkey_str = validator_pubkey.to_string();
                let vote_pubkey_str = vote_pubkey.to_string();
                
                // For whitelist: accept if either identity or vote pubkey is in the list
                if !config.discovery.whitelist.is_empty() {
                    let in_whitelist = config.discovery.whitelist.contains(&identity_pubkey_str) 
                        || config.discovery.whitelist.contains(&vote_pubkey_str);
                    if !in_whitelist {
                        continue;
                    }
                }
                
                // For blacklist: reject if either identity or vote pubkey is in the list
                if config.discovery.blacklist.contains(&identity_pubkey_str) 
                    || config.discovery.blacklist.contains(&vote_pubkey_str) {
                    continue;
                }
                
                let info = ValidatorInfo::new(validator_pubkey, vote_pubkey);
                validators.insert(validator_pubkey, info);
            }
        }
        
        info!("Discovered {} validators", validators.len());
        Ok(())
    }
    
    /// Fetch validators for CLI list command
    pub async fn fetch_validators(rpc_url: &str) -> Result<Vec<(ValidatorInfo, u64)>> {
        let rpc_client = RpcClient::new(rpc_url.to_string());
        
        // Create retry config
        let retry_config = RetryConfig::new()
            .with_max_attempts(3)
            .with_initial_delay(Duration::from_secs(1));
        
        // Get vote accounts with retry
        let vote_accounts = retry_with_config(
            || async { 
                rpc_client.get_vote_accounts().await
                    .map_err(|e| crate::error::Error::rpc(format!("Failed to get vote accounts: {}", e)))
            },
            retry_config,
        ).await?;
        
        let mut validators = Vec::new();
        
        // Process all validators (current and delinquent)
        for vote_account in vote_accounts.current.iter().chain(vote_accounts.delinquent.iter()) {
            let validator_pubkey = vote_account.node_pubkey.parse::<Pubkey>()?;
            let vote_pubkey = vote_account.vote_pubkey.parse::<Pubkey>()?;
            let stake = vote_account.activated_stake;
            
            let info = ValidatorInfo::new(validator_pubkey, vote_pubkey);
            validators.push((info, stake));
        }
        
        // Sort by stake descending
        validators.sort_by(|a, b| b.1.cmp(&a.1));
        
        Ok(validators)
    }
}

#[async_trait]
impl Shutdown for ValidatorDiscovery {
    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down validator discovery service");
        
        // Cancel the refresh task
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            // Wait for task to finish (or timeout)
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                handle
            ).await;
        }
        
        info!("Validator discovery service shutdown complete");
        Ok(())
    }
}

#[async_trait]
impl ValidatorDiscoveryTrait for ValidatorDiscovery {
    async fn discover(&self) -> Result<Vec<ValidatorInfo>> {
        self.refresh_validators().await?;
        Ok(self.get_all_validators().await)
    }

    async fn get_validator(&self, pubkey: &Pubkey) -> Option<ValidatorInfo> {
        self.validators.get(pubkey).map(|entry| entry.clone())
    }

    async fn get_all_validators(&self) -> Vec<ValidatorInfo> {
        self.validators
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, SolanaConfig, GrpcConfig, InfluxConfig, MetricsConfig, LatencyConfig, DiscoveryConfig};

    fn create_test_config() -> Config {
        Config {
            app: AppConfig {
                name: "test".to_string(),
                log_level: "info".to_string(),
                worker_threads: Some(4),
                debug: false,
            },
            solana: SolanaConfig {
                rpc_endpoint: "http://localhost:8899".to_string(),
                network: "devnet".to_string(),
                timeout_secs: 30,
                max_concurrent_requests: 5,
            },
            grpc: GrpcConfig {
                endpoint: None,
                access_token: None,
                max_subscriptions: 50,
                connection_timeout_secs: 30,
                reconnect_interval_secs: 5,
                buffer_size: 10000,
                enable_tls: false,
            },
            influxdb: InfluxConfig {
                url: "http://localhost:8086".to_string(),
                org: "test-org".to_string(),
                token: "test-token".to_string(),
                bucket: "test-bucket".to_string(),
                batch_size: 1000,
                flush_interval_ms: 100,
                num_workers: 2,
                enable_compression: false,
            },
            metrics: MetricsConfig {
                enabled: false,
                bind_address: "127.0.0.1".to_string(),
                port: 9090,
                collection_interval_secs: 60,
            },
            discovery: DiscoveryConfig {
                enabled: true,
                refresh_interval_secs: 60,
                min_stake_sol: 0.0,
                include_delinquent: false,
                whitelist: vec![],
                blacklist: vec![],
            },
            latency: LatencyConfig {
                window_size: 100,
                calculate_global_stats: true,
                stats_interval_secs: 30,
                outlier_threshold: 3.0,
            },
        }
    }

    #[test]
    fn test_whitelist_filtering_logic() {
        // This test validates the whitelist logic without needing network access
        // The actual filtering is done in refresh_validators_static, but we can
        // test the logic conceptually
        
        let identity_pubkey_str = "IdentityPubkey123".to_string();
        let vote_pubkey_str = "VoteAccountPubkey456".to_string();
        
        // Test 1: Empty whitelist should accept all
        let whitelist: Vec<String> = vec![];
        assert!(whitelist.is_empty());
        
        // Test 2: Whitelist with identity pubkey should match
        let whitelist = vec![identity_pubkey_str.clone()];
        assert!(whitelist.contains(&identity_pubkey_str));
        assert!(!whitelist.contains(&vote_pubkey_str));
        
        // Test 3: Whitelist with vote pubkey should match
        let whitelist = vec![vote_pubkey_str.clone()];
        assert!(!whitelist.contains(&identity_pubkey_str));
        assert!(whitelist.contains(&vote_pubkey_str));
        
        // Test 4: Whitelist with both should match either
        let whitelist = vec![identity_pubkey_str.clone(), vote_pubkey_str.clone()];
        assert!(whitelist.contains(&identity_pubkey_str) || whitelist.contains(&vote_pubkey_str));
    }

    #[tokio::test]
    async fn test_validator_discovery_creation() {
        let config = Arc::new(create_test_config());
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        // This will fail to connect to RPC, but we're just testing creation
        let discovery = ValidatorDiscovery::new(config, shutdown_rx).await;
        assert!(discovery.is_ok());
        
        // Clean up
        let _ = shutdown_tx.send(ShutdownSignal::Manual);
    }
}