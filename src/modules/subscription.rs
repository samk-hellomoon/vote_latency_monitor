//! gRPC Subscription Manager Module
//!
//! This module manages gRPC connections to validator nodes and handles
//! subscription to their transaction streams. It maintains active connections,
//! handles reconnections, and distributes incoming transactions to parsers.

use crate::error::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use futures::stream::StreamExt;
use futures::SinkExt;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::models::{ValidatorInfo, VoteTransaction};
use crate::modules::{Shutdown, ShutdownSignal};

// Use the official Yellowstone gRPC client
use yellowstone_grpc_client::{
    GeyserGrpcClient, 
    ClientTlsConfig, 
};
use yellowstone_grpc_proto::{
    geyser::{
        SubscribeRequest,
        SubscribeRequestFilterTransactions,
        SubscribeRequestFilterSlots,
        SubscribeRequestFilterAccounts,
        SubscribeUpdate,
        subscribe_update::UpdateOneof,
        CommitmentLevel,
    },
};
use tonic::Status;

/// Trait for subscription management
#[async_trait]
pub trait SubscriptionManagerTrait: Send + Sync {
    /// Subscribe to a validator's transaction stream
    async fn subscribe(&self, validator: &ValidatorInfo) -> Result<()>;
    
    /// Unsubscribe from a validator
    async fn unsubscribe(&self, pubkey: &Pubkey) -> Result<()>;
    
    /// Get the number of active subscriptions
    async fn active_subscriptions(&self) -> usize;
}

/// gRPC subscription manager
pub struct SubscriptionManager {
    config: Arc<Config>,
    active_connections: Arc<DashMap<Pubkey, JoinHandle<()>>>,
    tx_channel: mpsc::Sender<VoteTransaction>,
    rx_channel: Option<mpsc::Receiver<VoteTransaction>>,
    shutdown_rx: Option<tokio::sync::broadcast::Receiver<ShutdownSignal>>,
    grpc_endpoint: String,
    /// Tracks the global highest slot atomically
    highest_slot: Arc<std::sync::atomic::AtomicU64>,
}

impl SubscriptionManager {
    /// Get the gRPC endpoint
    pub fn grpc_endpoint(&self) -> &str {
        &self.grpc_endpoint
    }
    
    /// Get the highest slot seen so far
    pub fn get_highest_slot(&self) -> u64 {
        self.highest_slot.load(std::sync::atomic::Ordering::Acquire)
    }
    
    /// Run the actual subscription (separated for easier error handling)
    async fn run_subscription(
        validator: &ValidatorInfo,
        tx_channel: mpsc::Sender<VoteTransaction>,
        config: Arc<Config>,
        grpc_endpoint: String,
        highest_slot: Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<()> {
        // Create gRPC connection using the official client
        let endpoint_url = validator.grpc_endpoint.as_ref()
            .unwrap_or(&grpc_endpoint);
        
        info!("Connecting to gRPC endpoint: {}", endpoint_url);
        
        // Build client with authentication if provided
        let client_builder = GeyserGrpcClient::build_from_shared(endpoint_url.to_string())
            .map_err(|e| crate::error::Error::internal(format!("Invalid endpoint: {}", e)))?;
        
        let client_builder = if let Some(access_token) = &config.grpc.access_token {
            if !access_token.trim().is_empty() {
                debug!("Adding x-token authentication");
                client_builder.x_token(Some(access_token.trim().to_string()))
                    .map_err(|e| crate::error::Error::internal(format!("Invalid access token: {}", e)))?
            } else {
                warn!("Access token is empty, connecting without authentication");
                client_builder
            }
        } else {
            debug!("No access token provided, connecting without authentication");
            client_builder
        };
        
        let mut client = client_builder
            .connect_timeout(Duration::from_secs(config.grpc.connection_timeout_secs))
            .timeout(Duration::from_secs(config.grpc.connection_timeout_secs))
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .map_err(|e| crate::error::Error::internal(format!("TLS config error: {}", e)))?
            .max_decoding_message_size(1024 * 1024 * 1024) // 1GB max message size
            .connect()
            .await
            .map_err(|e| crate::error::Error::network(format!("Failed to connect: {}", e)))?;
        
        // Create subscription
        let (mut subscribe_tx, subscribe_rx) = client.subscribe().await
            .map_err(|e| crate::error::Error::network(format!("Failed to create subscription: {}", e)))?;
        
        // Create subscription request for vote transactions
        let request = Self::create_vote_subscription_request_static(&validator.vote_account);
        
        // Send the subscription request
        subscribe_tx.send(request).await
            .map_err(|e| crate::error::Error::network(format!("Failed to send subscription request: {}", e)))?;
        
        info!("Successfully subscribed to validator {} vote updates", validator.pubkey);
        
        // Handle the stream
        Self::handle_stream_static(validator.clone(), subscribe_rx, tx_channel, highest_slot).await
    }
    
    /// Static version of create_vote_subscription_request for use in static context
    fn create_vote_subscription_request_static(vote_pubkey: &Pubkey) -> SubscribeRequest {
        // Create filter for vote transactions (as backup/verification)
        let tx_filter = SubscribeRequestFilterTransactions {
            vote: Some(true),
            failed: Some(false),
            account_include: vec![vote_pubkey.to_string()],
            ..Default::default()
        };
        
        let mut tx_map = HashMap::new();
        tx_map.insert("vote_transactions".to_string(), tx_filter);
        
        // Create filter for slot updates (we need ALL slots to track current slot)
        let slot_filter = SubscribeRequestFilterSlots {
            filter_by_commitment: Some(true),
            interslot_updates: Some(false), // We only need finalized slot updates
        };
        
        let mut slot_map = HashMap::new();
        slot_map.insert("all_slots".to_string(), slot_filter);
        
        // Create filter for vote account updates
        let account_filter = SubscribeRequestFilterAccounts {
            account: vec![vote_pubkey.to_string()],
            owner: vec![], // Vote accounts are owned by the Vote program
            filters: vec![],
            nonempty_txn_signature: Some(false), // We want all account updates
        };
        
        let mut account_map = HashMap::new();
        account_map.insert("vote_account".to_string(), account_filter);
        
        SubscribeRequest {
            transactions: tx_map,
            slots: slot_map,
            accounts: account_map,
            commitment: Some(CommitmentLevel::Processed as i32),
            ..Default::default()
        }
    }
    /// Create a new subscription manager
    pub async fn new(
        config: Arc<Config>,
        shutdown_rx: tokio::sync::broadcast::Receiver<ShutdownSignal>,
    ) -> Result<Self> {
        // Create channel for vote transactions
        let (tx_channel, rx_channel) = mpsc::channel(config.grpc.buffer_size);
        
        // Determine gRPC endpoint with the following priority:
        // 1. Environment variable SVLM_GRPC_ENDPOINT
        // 2. Config file grpc.endpoint
        // 3. Derive from RPC endpoint
        let grpc_endpoint = if let Ok(endpoint) = std::env::var("SVLM_GRPC_ENDPOINT") {
            info!("Using gRPC endpoint from environment variable");
            endpoint
        } else if let Some(endpoint) = &config.grpc.endpoint {
            info!("Using gRPC endpoint from config");
            endpoint.clone()
        } else {
            info!("Deriving gRPC endpoint from RPC endpoint");
            // Derive from RPC endpoint if no explicit gRPC endpoint is provided
            let rpc_endpoint = &config.solana.rpc_endpoint;
            
            // Parse the URL to handle existing ports properly
            if let Ok(url) = url::Url::parse(rpc_endpoint) {
                let host = url.host_str().unwrap_or("localhost");
                let scheme = url.scheme();
                
                // If the RPC endpoint already has a non-standard port, it might be a gRPC endpoint
                // For example: https://example.com:2083 might already be pointing to gRPC
                if url.port().is_some() && url.port() != Some(443) && url.port() != Some(80) {
                    // Keep the existing URL as-is, preserving the scheme (http/https)
                    let path = url.path();
                    // Remove trailing slash if it's just "/"
                    let path = if path == "/" { "" } else { path };
                    format!("{}://{}:{}{}", 
                        scheme,
                        host, 
                        url.port().unwrap(),
                        path)
                } else {
                    // Standard RPC endpoint - add default gRPC port
                    // Use http by default for standard gRPC
                    format!("http://{}:10000", host)
                }
            } else {
                // Fallback for non-URL format
                format!("http://{}:10000", rpc_endpoint)
            }
        };
        
        info!("gRPC endpoint: {}", grpc_endpoint);
        
        Ok(Self {
            config,
            active_connections: Arc::new(DashMap::new()),
            tx_channel,
            rx_channel: Some(rx_channel),
            shutdown_rx: Some(shutdown_rx),
            grpc_endpoint,
            highest_slot: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Get the receiver channel for vote transactions
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<VoteTransaction>> {
        self.rx_channel.take()
    }
    
    /// Start managing subscriptions
    pub async fn start(&self) -> Result<()> {
        info!("Starting subscription manager");
        
        // TODO: Start health check task
        self.start_health_check().await?;
        
        Ok(())
    }

    /// Start health check task
    async fn start_health_check(&self) -> Result<()> {
        let connections = Arc::clone(&self.active_connections);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(30)
            );
            
            loop {
                interval.tick().await;
                debug!("Active gRPC connections: {}", connections.len());
                
                // TODO: Check connection health and reconnect if needed
            }
        });
        
        Ok(())
    }


    /// Handle incoming updates from gRPC stream (static version)
    async fn handle_stream_static(
        validator: ValidatorInfo,
        mut stream: impl futures::Stream<Item = std::result::Result<SubscribeUpdate, Status>> + Unpin,
        tx_channel: mpsc::Sender<VoteTransaction>,
        highest_slot: Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<()> {
        info!("Starting to handle stream for validator {}", validator.pubkey);
        
        while let Some(update_result) = stream.next().await {
            match update_result {
                Ok(update) => {
                    if let Some(update_oneof) = update.update_oneof {
                        match update_oneof {
                            UpdateOneof::Transaction(tx_update) => {
                                if let Some(tx_info) = tx_update.transaction {
                                    if tx_info.is_vote {
                                        debug!(
                                            "Received vote transaction from validator {}",
                                            validator.pubkey
                                        );
                                        
                                        // Parse the vote transaction directly using the Yellowstone data
                                        match crate::modules::parser::parse_yellowstone_vote_transaction(
                                            &tx_info,
                                            validator.pubkey,
                                            validator.vote_account,
                                            tx_update.slot,
                                        ) {
                                            Ok(vote_latency) => {
                                                debug!(
                                                    "Parsed vote transaction: slot={}, latency={}ms",
                                                    vote_latency.slot,
                                                    vote_latency.latency_ms
                                                );
                                                
                                                // Send the parsed vote latency directly to storage
                                                // Note: We need to update the channel type or create a new channel
                                                // For now, let's create a VoteTransaction for compatibility
                                                let vote_tx = VoteTransaction {
                                                    signature: vote_latency.signature.clone(),
                                                    validator_pubkey: validator.pubkey,
                                                    vote_pubkey: validator.vote_account,
                                                    slot: tx_update.slot,
                                                    timestamp: chrono::Utc::now(),
                                                    raw_data: Vec::new(),
                                                    voted_on_slots: vote_latency.voted_on_slots.clone(),
                                                    landed_slot: Some(vote_latency.landed_slot),
                                                };
                                                
                                                // Send to processing channel
                                                if let Err(e) = tx_channel.send(vote_tx).await {
                                                    error!("Failed to send vote transaction: {}", e);
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to parse vote transaction: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            UpdateOneof::Account(account_update) => {
                                debug!(
                                    "Received account update for slot {} from validator {}",
                                    account_update.slot,
                                    validator.pubkey
                                );
                                
                                // Account updates are logged for debugging but not used for latency calculation
                                // The slot in an account update is when the account was updated, not when
                                // the vote transaction landed, so it's not accurate for latency measurement.
                                
                                if let Some(account_info) = &account_update.account {
                                    // Check if this is the vote account we're interested in
                                    if let Ok(pubkey) = Pubkey::try_from(account_info.pubkey.as_slice()) {
                                        if pubkey == validator.vote_account {
                                            debug!(
                                                "Vote account update for validator {} at slot {} (for tracking only)",
                                                validator.pubkey,
                                                account_update.slot
                                            );
                                            
                                            // We could parse vote state here for debugging/tracking purposes
                                            // but we don't use it for latency calculation
                                            match crate::modules::parser::parse_vote_account_data(
                                                &account_info.data,
                                                validator.pubkey,
                                                validator.vote_account,
                                                account_update.slot,
                                            ) {
                                                Ok(_) => {
                                                    debug!("Successfully parsed vote account state");
                                                }
                                                Err(e) => {
                                                    debug!("Failed to parse vote account data: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            UpdateOneof::Slot(slot_update) => {
                                debug!(
                                    "Received slot update: slot={}, status={}",
                                    slot_update.slot,
                                    slot_update.status
                                );
                                
                                // Update the highest slot atomically - only move forward
                                // Use compare-and-swap to ensure we only update if this is a higher slot
                                let mut current = highest_slot.load(std::sync::atomic::Ordering::Acquire);
                                loop {
                                    if slot_update.slot <= current {
                                        // This slot is not higher, no update needed
                                        break;
                                    }
                                    
                                    match highest_slot.compare_exchange_weak(
                                        current,
                                        slot_update.slot,
                                        std::sync::atomic::Ordering::Release,
                                        std::sync::atomic::Ordering::Acquire,
                                    ) {
                                        Ok(_) => {
                                            debug!("Updated highest slot from {} to {}", current, slot_update.slot);
                                            break;
                                        }
                                        Err(actual) => {
                                            // Another thread updated the value, retry with the new value
                                            current = actual;
                                        }
                                    }
                                }
                            }
                            UpdateOneof::Ping(_ping) => {
                                debug!("Received ping from validator {}", validator.pubkey);
                                // Could implement pong response here if needed
                            }
                            _ => {
                                // Other update types not needed for MVP
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Error receiving from validator {}: {}",
                        validator.pubkey, e
                    );
                    return Err(crate::error::Error::network(format!(
                        "Stream error: {}",
                        e
                    )));
                }
            }
        }
        
        warn!("Stream ended for validator {}", validator.pubkey);
        Ok(())
    }
}

#[async_trait]
impl SubscriptionManagerTrait for SubscriptionManager {
    async fn subscribe(&self, validator: &ValidatorInfo) -> Result<()> {
        info!("Subscribing to validator: {}", validator.pubkey);
        
        // Check if already subscribed
        if self.active_connections.contains_key(&validator.pubkey) {
            debug!("Already subscribed to validator: {}", validator.pubkey);
            return Ok(());
        }
        
        // Clone necessary data for the spawned task
        let validator_clone = validator.clone();
        let tx_channel = self.tx_channel.clone();
        let config = Arc::clone(&self.config);
        let connections = Arc::clone(&self.active_connections);
        let grpc_endpoint = self.grpc_endpoint.clone();
        let highest_slot = Arc::clone(&self.highest_slot);
        
        // Spawn subscription task
        let handle = tokio::spawn(async move {
            loop {
                match Self::run_subscription(&validator_clone, tx_channel.clone(), config.clone(), grpc_endpoint.clone(), highest_slot.clone()).await {
                    Ok(_) => {
                        info!("Subscription ended normally for validator {}", validator_clone.pubkey);
                        break;
                    }
                    Err(e) => {
                        error!(
                            "Subscription error for validator {}: {}",
                            validator_clone.pubkey, e
                        );
                        
                        // Wait before reconnecting
                        tokio::time::sleep(tokio::time::Duration::from_secs(
                            config.grpc.reconnect_interval_secs
                        )).await;
                        
                        info!("Attempting to reconnect to validator {}", validator_clone.pubkey);
                    }
                }
            }
            
            // Remove from active connections when done
            connections.remove(&validator_clone.pubkey);
        });
        
        self.active_connections.insert(validator.pubkey, handle);
        
        Ok(())
    }

    async fn unsubscribe(&self, pubkey: &Pubkey) -> Result<()> {
        info!("Unsubscribing from validator: {}", pubkey);
        
        if let Some((_, handle)) = self.active_connections.remove(pubkey) {
            handle.abort();
            debug!("Unsubscribed from validator: {}", pubkey);
        }
        
        Ok(())
    }

    async fn active_subscriptions(&self) -> usize {
        self.active_connections.len()
    }
}

#[async_trait]
impl Shutdown for SubscriptionManager {
    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down subscription manager");
        
        // Cancel all active connections
        for entry in self.active_connections.iter() {
            entry.value().abort();
        }
        
        // Wait for all tasks to finish
        let handles: Vec<_> = self.active_connections
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
            
        for pubkey in handles {
            if let Some((_, handle)) = self.active_connections.remove(&pubkey) {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    handle
                ).await;
            }
        }
        
        info!("Subscription manager shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscription_manager_creation() {
        let config = Arc::new(Config::default());
        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let _manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
    }

    #[test]
    fn test_header_handling_with_empty_token() {
        // Test that empty access tokens are handled correctly
        let access_token = "";
        
        // This should not panic (previously would panic with index out of bounds)
        if !access_token.trim().is_empty() {
            match tonic::metadata::MetadataValue::try_from(access_token.trim()) {
                Ok(_) => {
                    // Should not reach here with empty token
                    panic!("Empty token should not create valid header");
                }
                Err(_) => {
                    // Expected behavior - empty token should fail
                }
            }
        }
        // Test passes if we reach here without panic
    }

    #[test]
    fn test_header_handling_with_valid_token() {
        // Test that valid access tokens work correctly
        let access_token = "valid_token_123";
        
        if !access_token.trim().is_empty() {
            match tonic::metadata::MetadataValue::try_from(access_token.trim()) {
                Ok(header_value) => {
                    // Should succeed with valid token
                    assert_eq!(header_value.to_str().unwrap(), "valid_token_123");
                }
                Err(e) => {
                    panic!("Valid token should create valid header: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_header_handling_with_whitespace_token() {
        // Test that tokens with whitespace are trimmed correctly
        let access_token = "  valid_token_with_spaces  ";
        
        if !access_token.trim().is_empty() {
            match tonic::metadata::MetadataValue::try_from(access_token.trim()) {
                Ok(header_value) => {
                    // Should succeed with trimmed token
                    assert_eq!(header_value.to_str().unwrap(), "valid_token_with_spaces");
                }
                Err(e) => {
                    panic!("Valid token with whitespace should create valid header: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_header_handling_with_invalid_token() {
        // Test that invalid tokens are handled gracefully
        let access_token = "invalid\ntoken\r\n";
        
        if !access_token.trim().is_empty() {
            match tonic::metadata::MetadataValue::try_from(access_token.trim()) {
                Ok(_) => {
                    // This might succeed or fail depending on the token format
                    // The important thing is that it doesn't panic
                }
                Err(_) => {
                    // Expected behavior - invalid token should fail gracefully
                }
            }
        }
        // Test passes if we reach here without panic
    }
}

#[cfg(test)]
#[path = "subscription_test.rs"]
mod subscription_test;