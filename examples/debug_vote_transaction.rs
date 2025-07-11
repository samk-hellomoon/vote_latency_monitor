//! Debug program to inspect Yellowstone vote transaction structure
//! 
//! This program connects to Yellowstone gRPC and prints all available fields
//! from vote transaction updates to understand what data is available for
//! extracting voted-on slots.

use anyhow::Result;
use futures::StreamExt;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::env;
use tracing::{debug, error, info, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterTransactions, SubscribeUpdate,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("debug_vote_transaction=debug".parse()?)
                .add_directive("yellowstone_grpc_client=debug".parse()?),
        )
        .init();

    // Get configuration from environment or use defaults
    let endpoint = env::var("SVLM_GRPC_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:10000".to_string());
    let access_token = env::var("SVLM_GRPC_ACCESS_TOKEN").ok();

    // Vote account to monitor (you can change this to any active vote account)
    let vote_account = env::var("DEBUG_VOTE_ACCOUNT")
        .unwrap_or_else(|_| "CertusDeBmqN8ZawdkxK5kFGMwBXdudvWHYwtNgNhvLu".to_string());

    info!("Connecting to Yellowstone gRPC endpoint: {}", endpoint);
    info!("Monitoring vote account: {}", vote_account);

    // Build gRPC client
    let mut client_builder = GeyserGrpcClient::build_from_shared(endpoint)?;

    if let Some(token) = access_token {
        info!("Using authentication token");
        client_builder = client_builder.x_token(Some(token))?;
    }

    let mut client = client_builder
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .max_decoding_message_size(1024 * 1024 * 1024) // 1GB
        .connect()
        .await?;

    info!("Connected to gRPC endpoint");

    // Create subscription
    let (mut subscribe_tx, subscribe_rx) = client.subscribe().await?;

    // Create subscription request for vote transactions
    let tx_filter = SubscribeRequestFilterTransactions {
        vote: Some(true),
        failed: Some(false),
        account_include: vec![vote_account.clone()],
        ..Default::default()
    };

    let mut tx_map = HashMap::new();
    tx_map.insert("vote_transactions".to_string(), tx_filter);

    let request = SubscribeRequest {
        transactions: tx_map,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    };

    // Send subscription request
    subscribe_tx.send(request).await?;
    info!("Subscription request sent, waiting for vote transactions...");

    // Process updates
    let mut stream = subscribe_rx;
    let mut transaction_count = 0;
    let max_transactions = env::var("DEBUG_MAX_TRANSACTIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    while let Some(update_result) = stream.next().await {
        match update_result {
            Ok(update) => {
                if let Some(update_oneof) = update.update_oneof {
                    match update_oneof {
                        UpdateOneof::Transaction(tx_update) => {
                            transaction_count += 1;
                            info!("===============================================");
                            info!("TRANSACTION UPDATE #{}", transaction_count);
                            info!("===============================================");
                            
                            // Print all fields from tx_update
                            info!("Slot: {}", tx_update.slot);
                            
                            if let Some(tx_info) = &tx_update.transaction {
                                info!("\nSubscribeUpdateTransactionInfo fields:");
                                info!("  - signature: {} ({})", 
                                    bs58::encode(&tx_info.signature).into_string(),
                                    tx_info.signature.len()
                                );
                                info!("  - is_vote: {}", tx_info.is_vote);
                                info!("  - transaction: {:?}", tx_info.transaction.is_some());
                                info!("  - meta: {:?}", tx_info.meta.is_some());
                                info!("  - index: {}", tx_info.index);
                                
                                // Inspect the transaction field
                                if let Some(tx) = &tx_info.transaction {
                                    info!("\nTransaction fields:");
                                    info!("  - signatures: {} signatures", tx.signatures.len());
                                    for (i, sig) in tx.signatures.iter().enumerate() {
                                        info!("    [{}]: {}", i, bs58::encode(sig).into_string());
                                    }
                                    
                                    if let Some(message) = &tx.message {
                                        info!("\n  Message fields:");
                                        info!("    - header: {:?}", message.header);
                                        info!("    - account_keys: {} keys", message.account_keys.len());
                                        for (i, key) in message.account_keys.iter().enumerate() {
                                            info!("      [{}]: {}", i, bs58::encode(key).into_string());
                                        }
                                        info!("    - recent_blockhash: {}", bs58::encode(&message.recent_blockhash).into_string());
                                        info!("    - instructions: {} instructions", message.instructions.len());
                                        
                                        // Print instruction details
                                        for (i, inst) in message.instructions.iter().enumerate() {
                                            info!("\n    Instruction [{}]:", i);
                                            info!("      - program_id_index: {}", inst.program_id_index);
                                            info!("      - accounts: {:?}", inst.accounts);
                                            info!("      - data length: {} bytes", inst.data.len());
                                            
                                            // Print first 100 bytes of instruction data as hex
                                            let data_preview = if inst.data.len() > 100 {
                                                &inst.data[..100]
                                            } else {
                                                &inst.data
                                            };
                                            info!("      - data (hex): {}", hex::encode(data_preview));
                                            if inst.data.len() > 100 {
                                                info!("        ... {} more bytes", inst.data.len() - 100);
                                            }
                                            
                                            // Try to decode as vote instruction
                                            if let Ok(vote_inst) = bincode::deserialize::<solana_sdk::vote::instruction::VoteInstruction>(&inst.data) {
                                                info!("      - Decoded as VoteInstruction: {:?}", vote_inst);
                                                
                                                // Extract slots based on instruction type
                                                match vote_inst {
                                                    solana_sdk::vote::instruction::VoteInstruction::Vote(vote) => {
                                                        info!("        Vote slots: {:?}", vote.slots);
                                                        info!("        Vote hash: {}", vote.hash);
                                                        info!("        Vote timestamp: {:?}", vote.timestamp);
                                                    }
                                                    solana_sdk::vote::instruction::VoteInstruction::VoteSwitch(vote, _) => {
                                                        info!("        VoteSwitch slots: {:?}", vote.slots);
                                                        info!("        VoteSwitch hash: {}", vote.hash);
                                                        info!("        VoteSwitch timestamp: {:?}", vote.timestamp);
                                                    }
                                                    solana_sdk::vote::instruction::VoteInstruction::UpdateVoteState(update) => {
                                                        let slots: Vec<u64> = update.lockouts.iter()
                                                            .map(|l| l.slot())
                                                            .collect();
                                                        info!("        UpdateVoteState slots: {:?}", slots);
                                                        info!("        UpdateVoteState hash: {}", update.hash);
                                                        info!("        UpdateVoteState timestamp: {:?}", update.timestamp);
                                                    }
                                                    solana_sdk::vote::instruction::VoteInstruction::UpdateVoteStateSwitch(update, _) => {
                                                        let slots: Vec<u64> = update.lockouts.iter()
                                                            .map(|l| l.slot())
                                                            .collect();
                                                        info!("        UpdateVoteStateSwitch slots: {:?}", slots);
                                                        info!("        UpdateVoteStateSwitch hash: {}", update.hash);
                                                        info!("        UpdateVoteStateSwitch timestamp: {:?}", update.timestamp);
                                                    }
                                                    _ => {
                                                        info!("        Other vote instruction type");
                                                    }
                                                }
                                            } else {
                                                debug!("      - Could not decode as VoteInstruction");
                                            }
                                        }
                                        
                                        info!("    - address_table_lookups: {} lookups", message.address_table_lookups.len());
                                        info!("    - versioned: {}", message.versioned);
                                    }
                                }
                                
                                // Inspect the meta field
                                if let Some(meta) = &tx_info.meta {
                                    info!("\nTransactionStatusMeta fields:");
                                    info!("  - err: {:?}", meta.err);
                                    info!("  - fee: {}", meta.fee);
                                    info!("  - pre_balances: {:?}", meta.pre_balances);
                                    info!("  - post_balances: {:?}", meta.post_balances);
                                    info!("  - inner_instructions: {} groups", meta.inner_instructions.len());
                                    info!("  - log_messages: {} messages", meta.log_messages.len());
                                    for (i, log) in meta.log_messages.iter().take(10).enumerate() {
                                        info!("    [{}]: {}", i, log);
                                    }
                                    if meta.log_messages.len() > 10 {
                                        info!("    ... {} more log messages", meta.log_messages.len() - 10);
                                    }
                                    info!("  - pre_token_balances: {} balances", meta.pre_token_balances.len());
                                    info!("  - post_token_balances: {} balances", meta.post_token_balances.len());
                                    info!("  - rewards: {} rewards", meta.rewards.len());
                                    info!("  - loaded_addresses: {:?}", meta.loaded_addresses.is_some());
                                    info!("  - return_data: {:?}", meta.return_data.is_some());
                                    info!("  - compute_units_consumed: {:?}", meta.compute_units_consumed);
                                }
                            }
                            
                            info!("===============================================\n");
                            
                            if transaction_count >= max_transactions {
                                info!("Reached maximum transaction count ({}), exiting...", max_transactions);
                                break;
                            }
                        }
                        UpdateOneof::Ping(_) => {
                            debug!("Received ping");
                        }
                        _ => {
                            debug!("Received other update type");
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error receiving update: {}", e);
                break;
            }
        }
    }

    info!("Debug session complete. Processed {} transactions.", transaction_count);
    Ok(())
}

// Add hex dependency for the debug program
#[cfg(not(feature = "hex"))]
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}