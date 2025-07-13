//! Core modules for the Solana Vote Latency Monitor
//!
//! This module contains the main components of the monitoring system:
//! - Validator discovery
//! - gRPC subscription management
//! - Vote transaction parsing
//! - Latency calculation
//! - Storage management

pub mod calculator;
pub mod discovery;
pub mod parser;
pub mod storage;
pub mod subscription;

pub use calculator::LatencyCalculator;
pub use discovery::ValidatorDiscovery;
pub use parser::VoteParser;
pub use storage::StorageManagerTrait;
pub use subscription::SubscriptionManager;

use crate::config::Config;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, error};

/// Shutdown signal types
#[derive(Debug, Clone, Copy)]
pub enum ShutdownSignal {
    /// Ctrl+C was pressed
    CtrlC,
    /// SIGTERM received
    Sigterm,
    /// Manual shutdown requested
    Manual,
}

/// Trait for modules that can be shutdown gracefully
#[async_trait::async_trait]
pub trait Shutdown: Send + Sync {
    /// Perform graceful shutdown
    async fn shutdown(&mut self) -> Result<()>;
}

/// Manager for coordinating all modules
pub struct ModuleManager {
    config: Arc<Config>,
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
    storage: Option<Arc<dyn crate::modules::storage::StorageManagerTrait>>,
    discovery: Option<Arc<tokio::sync::RwLock<ValidatorDiscovery>>>,
    subscription: Option<Arc<tokio::sync::RwLock<SubscriptionManager>>>,
    calculator: Option<Arc<tokio::sync::RwLock<LatencyCalculator>>>,
}

impl ModuleManager {
    /// Create a new module manager
    pub fn new(config: Arc<Config>, shutdown_tx: broadcast::Sender<ShutdownSignal>) -> Self {
        Self {
            config,
            shutdown_tx,
            storage: None,
            discovery: None,
            subscription: None,
            calculator: None,
        }
    }
    
    /// Start all modules
    pub async fn start_all(&mut self) -> Result<()> {
        info!("Starting all modules...");
        
        // Initialize storage
        info!("Initializing storage module...");
        
        info!("Initializing InfluxDB storage...");
        let influxdb_storage = Arc::new(
            crate::storage::InfluxDBStorage::new(self.config.influxdb.clone()).await?
        );
        
        self.storage = Some(influxdb_storage as Arc<dyn crate::modules::storage::StorageManagerTrait>);
        info!("InfluxDB storage initialized successfully");
        
        // Initialize and start validator discovery
        if self.config.discovery.enabled {
            info!("Initializing validator discovery module...");
            let mut discovery = ValidatorDiscovery::new(
                self.config.clone(),
                self.shutdown_tx.subscribe(),
            ).await?;
            discovery.start().await?;
            self.discovery = Some(Arc::new(tokio::sync::RwLock::new(discovery)));
        }
        
        // Initialize and start subscription manager
        info!("Initializing subscription manager...");
        let subscription = SubscriptionManager::new(
            self.config.clone(),
            self.shutdown_tx.subscribe(),
        ).await?;
        subscription.start().await?;
        self.subscription = Some(Arc::new(tokio::sync::RwLock::new(subscription)));
        
        // Initialize and start latency calculator
        info!("Initializing latency calculator...");
        let mut calculator = LatencyCalculator::new(
            self.config.clone(),
            self.storage.clone(),
            self.shutdown_tx.subscribe(),
        ).await?;
        calculator.start().await?;
        self.calculator = Some(Arc::new(tokio::sync::RwLock::new(calculator)));
        
        info!("All modules started successfully");
        Ok(())
    }
    
    /// Stop all modules gracefully
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("Stopping all modules...");
        
        // Stop in reverse order
        if let Some(calculator) = &self.calculator {
            let mut calc = calculator.write().await;
            if let Err(e) = calc.shutdown().await {
                error!("Error shutting down calculator: {}", e);
            }
        }
        
        if let Some(subscription) = &self.subscription {
            let mut sub = subscription.write().await;
            if let Err(e) = sub.shutdown().await {
                error!("Error shutting down subscription manager: {}", e);
            }
        }
        
        if let Some(discovery) = &self.discovery {
            let mut disc = discovery.write().await;
            if let Err(e) = disc.shutdown().await {
                error!("Error shutting down discovery: {}", e);
            }
        }
        
        if let Some(_storage) = &self.storage {
            // Storage doesn't use RwLock, need to handle differently
            // For now, just log that we're done with storage
            info!("Closing storage connections");
        }
        
        info!("All modules stopped");
        Ok(())
    }
}