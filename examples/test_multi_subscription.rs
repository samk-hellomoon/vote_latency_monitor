use anyhow::Result;
use std::env;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use yellowstone_grpc_client::{GeyserGrpcClient, ClientTlsConfig};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterTransactions, SubscribeRequestFilterSlots,
    SubscribeRequestFilterAccounts,
};
use futures::StreamExt;
use std::collections::HashMap;
use solana_sdk::pubkey::Pubkey;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,yellowstone_grpc_client=debug")),
        )
        .init();

    // Get configuration from environment
    let endpoint = env::var("GRPC_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:10000".to_string());
    let access_token = env::var("GRPC_ACCESS_TOKEN").ok();

    // Vote account to monitor (you can change this to any vote account)
    let vote_account = env::var("VOTE_ACCOUNT")
        .unwrap_or_else(|_| "J2BLfpm7ch8xi4UH6vJLCy88sB3dgCCWqJS2d8FfQj3m".to_string());

    info!("Connecting to gRPC endpoint: {}", endpoint);
    info!("Monitoring vote account: {}", vote_account);

    // Connect to gRPC endpoint
    let client_builder = GeyserGrpcClient::build_from_shared(endpoint)?;
    
    let client_builder = if let Some(token) = access_token {
        if !token.trim().is_empty() {
            info!("Using access token for authentication");
            client_builder.x_token(Some(token.trim().to_string()))?
        } else {
            client_builder
        }
    } else {
        client_builder
    };

    let mut client = client_builder
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .connect()
        .await?;

    // Create subscription
    let (mut subscribe_tx, mut subscribe_rx) = client.subscribe().await?;

    // Create comprehensive subscription request
    let request = create_comprehensive_subscription(&vote_account);
    
    // Send subscription request
    subscribe_tx.send(request).await?;
    info!("Subscription request sent");

    // Track statistics
    let mut slot_updates = 0;
    let mut account_updates = 0;
    let mut transaction_updates = 0;
    let mut current_slot = 0u64;

    // Process updates
    while let Some(update_result) = subscribe_rx.next().await {
        match update_result {
            Ok(update) => {
                if let Some(update_oneof) = update.update_oneof {
                    match update_oneof {
                        UpdateOneof::Slot(slot_update) => {
                            slot_updates += 1;
                            current_slot = slot_update.slot;
                            info!(
                                "SLOT UPDATE #{}: slot={}, status={}, parent={:?}",
                                slot_updates,
                                slot_update.slot,
                                slot_update.status,
                                slot_update.parent
                            );
                        }
                        UpdateOneof::Account(account_update) => {
                            account_updates += 1;
                            if let Some(account_info) = &account_update.account {
                                let pubkey = bs58::encode(&account_info.pubkey).into_string();
                                info!(
                                    "ACCOUNT UPDATE #{}: pubkey={}, slot={}, data_len={}, lamports={}",
                                    account_updates,
                                    pubkey,
                                    account_update.slot,
                                    account_info.data.len(),
                                    account_info.lamports
                                );
                                
                                // Calculate latency based on current slot
                                if current_slot > account_update.slot {
                                    let latency = current_slot - account_update.slot;
                                    info!("  -> Account update latency: {} slots", latency);
                                }
                            }
                        }
                        UpdateOneof::Transaction(tx_update) => {
                            transaction_updates += 1;
                            if let Some(tx_info) = &tx_update.transaction {
                                let signature = bs58::encode(&tx_info.signature).into_string();
                                info!(
                                    "TRANSACTION UPDATE #{}: sig={}, slot={}, is_vote={}",
                                    transaction_updates,
                                    signature,
                                    tx_update.slot,
                                    tx_info.is_vote
                                );
                                
                                // Calculate transaction latency
                                if current_slot > tx_update.slot {
                                    let latency = current_slot - tx_update.slot;
                                    info!("  -> Transaction latency: {} slots", latency);
                                }
                            }
                        }
                        UpdateOneof::Ping(_) => {
                            info!("Received ping");
                        }
                        _ => {
                            // Other update types
                        }
                    }
                }
                
                // Print statistics every 10 updates
                let total = slot_updates + account_updates + transaction_updates;
                if total % 10 == 0 && total > 0 {
                    info!(
                        "\n--- Statistics ---\nSlot updates: {}\nAccount updates: {}\nTransaction updates: {}\nCurrent slot: {}\n-----------------\n",
                        slot_updates, account_updates, transaction_updates, current_slot
                    );
                }
            }
            Err(e) => {
                warn!("Error receiving update: {}", e);
                break;
            }
        }
    }

    info!("Stream ended");
    Ok(())
}

fn create_comprehensive_subscription(vote_account: &str) -> SubscribeRequest {
    // Create filter for vote transactions (as backup/verification)
    let tx_filter = SubscribeRequestFilterTransactions {
        vote: Some(true),
        failed: Some(false),
        account_include: vec![vote_account.to_string()],
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
        account: vec![vote_account.to_string()],
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