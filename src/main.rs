//! Solana Vote Latency Monitor (SVLM)
//! 
//! A monitoring system for tracking vote latency across Solana validators.
//! This tool subscribes to validator gRPC feeds, parses vote transactions,
//! and calculates latency metrics to help identify network performance issues.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::signal;
use tracing::{info, error, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use svlm::config::Config;
use svlm::modules::{ShutdownSignal, Shutdown};
use svlm::modules::discovery::ValidatorDiscoveryTrait;
use svlm::modules::subscription::SubscriptionManagerTrait;
use svlm::modules::parser::VoteParserTrait;
use svlm::modules::calculator::LatencyCalculatorTrait;

#[derive(Parser)]
#[command(
    name = "svlm",
    version,
    about = "Solana Vote Latency Monitor - Track vote latency across validators",
    long_about = None
)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", default_value = "config/config.toml")]
    config: PathBuf,

    /// Set the log level
    #[arg(short, long, env = "RUST_LOG", default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the monitoring system
    Run {
        /// Override the number of worker threads
        #[arg(long)]
        workers: Option<usize>,
    },
    /// Validate the configuration file
    ValidateConfig,
    /// List discovered validators
    ListValidators {
        /// RPC endpoint to query
        #[arg(long)]
        rpc_url: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level)?;

    // Load configuration
    let config = Config::load(&cli.config)?;
    info!("Loaded configuration from: {}", cli.config.display());

    // Handle commands
    match cli.command {
        Some(Commands::Run { workers }) => {
            info!("Starting Solana Vote Latency Monitor...");
            
            // Override worker count if specified
            if let Some(worker_count) = workers {
                info!("Using {} worker threads", worker_count);
                // TODO: Configure tokio runtime with specific worker count
            }

            // Initialize the monitoring system
            run_monitor(config).await?;
        }
        Some(Commands::ValidateConfig) => {
            info!("Configuration is valid");
            println!("{:#?}", config);
        }
        Some(Commands::ListValidators { rpc_url }) => {
            let endpoint = rpc_url.unwrap_or_else(|| config.solana.rpc_endpoint.clone());
            info!("Listing validators from: {}", endpoint);
            list_validators(&endpoint).await?;
        }
        None => {
            // Default to running the monitor
            info!("Starting Solana Vote Latency Monitor (default mode)...");
            run_monitor(config).await?;
        }
    }

    Ok(())
}

/// Initialize the logging system
fn init_logging(log_level: &str) -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

/// Run the main monitoring system
async fn run_monitor(config: Config) -> Result<()> {
    info!("Initializing monitoring system...");
    
    // Create shutdown broadcast channel
    let (shutdown_tx, _) = broadcast::channel::<ShutdownSignal>(1);
    let config = Arc::new(config);
    
    // Initialize storage
    info!("Initializing InfluxDB storage...");
    let storage = Arc::new(
        svlm::storage::InfluxDBStorage::new(config.influxdb.clone()).await?
    );
    info!("InfluxDB storage initialized successfully");
    
    // Step 2: Create and start the discovery module to fetch validators
    info!("Starting validator discovery...");
    let mut discovery = svlm::modules::discovery::ValidatorDiscovery::new(
        config.clone(),
        shutdown_tx.subscribe(),
    ).await?;
    
    // Perform initial discovery
    let validators = discovery.discover().await?;
    info!("Discovered {} validators", validators.len());
    
    // Start the discovery background task
    discovery.start().await?;
    let discovery = Arc::new(tokio::sync::RwLock::new(discovery));
    
    // Step 3: Initialize the parser
    info!("Initializing vote parser...");
    let parser = Arc::new(svlm::modules::parser::VoteParser::new()?);
    
    // Step 4: Initialize the calculator with storage
    info!("Initializing latency calculator...");
    let mut calculator = svlm::modules::calculator::LatencyCalculator::new(
        config.clone(),
        Some(storage.clone()),
        shutdown_tx.subscribe(),
    ).await?;
    calculator.start().await?;
    let calculator = Arc::new(tokio::sync::RwLock::new(calculator));
    
    // Step 5: Create and start the subscription manager
    info!("Initializing subscription manager...");
    let subscription_manager = svlm::modules::subscription::SubscriptionManager::new(
        config.clone(),
        shutdown_tx.subscribe(),
    ).await?;
    
    // Subscribe to all discovered validators
    let validator_count = validators.len();
    for validator in validators {
        if let Err(e) = subscription_manager.subscribe(&validator).await {
            error!("Failed to subscribe to validator {}: {}", validator.pubkey, e);
        }
    }
    
    subscription_manager.start().await?;
    let subscription_manager = Arc::new(tokio::sync::RwLock::new(subscription_manager));
    
    // Step 6: Wire up the data processing pipeline
    // Task 1: Process votes from subscription manager
    let parser_clone = parser.clone();
    let calculator_clone = calculator.clone();
    let storage = storage as Arc<dyn svlm::modules::storage::StorageManagerTrait>;
    let storage_clone = storage.clone();
    let subscription_manager_for_processor = Arc::clone(&subscription_manager);
    let vote_processor = tokio::spawn(async move {
        // Get the receiver from subscription manager
        let mut sub_manager = subscription_manager_for_processor.write().await;
        if let Some(mut receiver) = sub_manager.take_receiver() {
            drop(sub_manager); // Release the lock
            
            while let Some(vote_tx) = receiver.recv().await {
                // Parse the vote transaction
                match parser_clone.parse(&vote_tx).await {
                    Ok(vote_latency) => {
                        // Calculate metrics (non-blocking, just updates in-memory data)
                        let calc = calculator_clone.read().await;
                        if let Err(e) = calc.calculate(&vote_latency).await {
                            error!("Failed to calculate latency: {}", e);
                        }
                        drop(calc); // Release the lock immediately
                        
                        // Store in database using a separate task to avoid blocking the channel
                        let storage_for_task = storage_clone.clone();
                        let vote_latency_clone = vote_latency.clone();
                        tokio::spawn(async move {
                            if let Err(e) = storage_for_task.store_vote_latency(&vote_latency_clone).await {
                                error!("Failed to store vote latency: {}", e);
                            } else {
                                trace!("Stored vote latency for validator {} slot {}", 
                                    vote_latency_clone.validator_pubkey, vote_latency_clone.slot);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to parse vote transaction: {}", e);
                    }
                }
            }
        }
    });
    
    // Task 2: Periodically check for new validators
    let discovery_clone = discovery.clone();
    let subscription_manager_clone = Arc::clone(&subscription_manager);
    let validator_updater = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            
            let disc = discovery_clone.read().await;
            match disc.discover().await {
                Ok(new_validators) => {
                    drop(disc); // Release the lock
                    
                    let sub_mgr = subscription_manager_clone.write().await;
                    for validator in new_validators {
                        if let Err(e) = sub_mgr.subscribe(&validator).await {
                            error!("Failed to subscribe to new validator {}: {}", validator.pubkey, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to discover new validators: {}", e);
                }
            }
        }
    });
    
    info!("Monitoring system started successfully");
    info!("Processing votes from {} validators", validator_count);
    
    // Wait for shutdown signal
    let shutdown_reason = wait_for_shutdown_signal().await;
    info!("Received shutdown signal: {:?}", shutdown_reason);
    
    // Initiate graceful shutdown
    info!("Starting graceful shutdown...");
    
    // Send shutdown signal to all modules
    if let Err(e) = shutdown_tx.send(shutdown_reason) {
        error!("Failed to send shutdown signal: {}", e);
    }
    
    // Cancel background tasks
    vote_processor.abort();
    validator_updater.abort();
    
    // Stop all modules
    let mut sub_mgr = subscription_manager.write().await;
    if let Err(e) = sub_mgr.shutdown().await {
        error!("Error shutting down subscription manager: {}", e);
    }
    drop(sub_mgr);
    
    let mut calc = calculator.write().await;
    if let Err(e) = calc.shutdown().await {
        error!("Error shutting down calculator: {}", e);
    }
    drop(calc);
    
    let mut disc = discovery.write().await;
    if let Err(e) = disc.shutdown().await {
        error!("Error shutting down discovery: {}", e);
    }
    drop(disc);
    
    info!("Monitoring system stopped successfully");
    Ok(())
}

/// Wait for shutdown signals (SIGTERM, SIGINT, or Ctrl+C)
async fn wait_for_shutdown_signal() -> ShutdownSignal {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        ShutdownSignal::CtrlC
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
        ShutdownSignal::Sigterm
    };

    #[cfg(not(unix))]
    let terminate = async {
        // On non-Unix systems, we'll just wait for Ctrl+C
        std::future::pending::<()>().await;
        ShutdownSignal::CtrlC
    };

    tokio::select! {
        signal = ctrl_c => signal,
        signal = terminate => signal,
    }
}


/// List validators from the RPC endpoint
async fn list_validators(rpc_url: &str) -> Result<()> {
    use svlm::modules::discovery::ValidatorDiscovery;
    use svlm::retry::{retry_with_config, RetryConfig};
    use std::time::Duration;
    
    info!("Querying validators from: {}", rpc_url);
    
    // Create retry config for RPC operations
    let retry_config = RetryConfig::new()
        .with_max_attempts(3)
        .with_initial_delay(Duration::from_secs(1))
        .with_max_delay(Duration::from_secs(10));
    
    // Query validators with retry
    let validators = retry_with_config(
        || async { ValidatorDiscovery::fetch_validators(rpc_url).await },
        retry_config,
    )
    .await?;
    
    // Display validator information
    println!("\nDiscovered {} validators:\n", validators.len());
    println!("{:<44} {:<44} {:<20} {:<10}", "Identity", "Vote Account", "Name", "Stake (SOL)");
    println!("{}", "-".repeat(120));
    
    for (info, stake) in validators {
        let name = info.name.as_deref().unwrap_or("<unknown>");
        let stake_sol = stake as f64 / 1_000_000_000.0; // Convert lamports to SOL
        println!(
            "{:<44} {:<44} {:<20} {:<10.2}",
            info.pubkey.to_string(),
            info.vote_account.to_string(),
            name,
            stake_sol
        );
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}