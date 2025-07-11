//! Test program to verify vote transaction parsing from Yellowstone data
//! 
//! This program subscribes to a single vote transaction and attempts to
//! extract the voted-on slots using the transaction data provided by Yellowstone.

use anyhow::Result;
use futures::StreamExt;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::env;
use tracing::{debug, error, info};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterTransactions, SubscribeUpdate,
    SubscribeUpdateTransactionInfo,
};

/// Extract voted slots from a Yellowstone transaction update
fn extract_voted_slots(tx_info: &SubscribeUpdateTransactionInfo, landed_slot: u64) -> Result<Vec<u64>> {
    info!("Attempting to extract voted slots from transaction");
    
    let mut voted_slots = Vec::new();
    
    // Check if we have transaction data
    if let Some(tx) = &tx_info.transaction {
        if let Some(message) = &tx.message {
            // Iterate through instructions
            for (idx, instruction) in message.instructions.iter().enumerate() {
                info!("Processing instruction {}", idx);
                
                // Get the program ID for this instruction
                if let Some(program_key) = message.account_keys.get(instruction.program_id_index as usize) {
                    let program_id = bs58::encode(program_key).into_string();
                    info!("Program ID: {}", program_id);
                    
                    // Check if this is a vote program instruction
                    if program_id == "Vote111111111111111111111111111111111111111" {
                        info!("Found vote program instruction with {} bytes of data", instruction.data.len());
                        
                        // Try to deserialize the instruction data
                        match bincode::deserialize::<solana_sdk::vote::instruction::VoteInstruction>(&instruction.data) {
                            Ok(vote_inst) => {
                                match vote_inst {
                                    solana_sdk::vote::instruction::VoteInstruction::Vote(vote) => {
                                        info!("Decoded Vote instruction with slots: {:?}", vote.slots);
                                        voted_slots.extend(&vote.slots);
                                    }
                                    solana_sdk::vote::instruction::VoteInstruction::VoteSwitch(vote, _) => {
                                        info!("Decoded VoteSwitch instruction with slots: {:?}", vote.slots);
                                        voted_slots.extend(&vote.slots);
                                    }
                                    solana_sdk::vote::instruction::VoteInstruction::UpdateVoteState(update) => {
                                        let slots: Vec<u64> = update.lockouts.iter()
                                            .map(|l| l.slot())
                                            .collect();
                                        info!("Decoded UpdateVoteState instruction with slots: {:?}", slots);
                                        voted_slots.extend(&slots);
                                    }
                                    solana_sdk::vote::instruction::VoteInstruction::UpdateVoteStateSwitch(update, _) => {
                                        let slots: Vec<u64> = update.lockouts.iter()
                                            .map(|l| l.slot())
                                            .collect();
                                        info!("Decoded UpdateVoteStateSwitch instruction with slots: {:?}", slots);
                                        voted_slots.extend(&slots);
                                    }
                                    _ => {
                                        info!("Other vote instruction type (no slots)");
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize vote instruction: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Remove duplicates and sort
    voted_slots.sort_unstable();
    voted_slots.dedup();
    
    if voted_slots.is_empty() {
        warn!("No voted slots found, using landed slot as fallback");
        voted_slots.push(landed_slot);
    }
    
    Ok(voted_slots)
}

/// Calculate latencies for voted slots
fn calculate_latencies(voted_slots: &[u64], landed_slot: u64) -> Vec<u64> {
    voted_slots.iter()
        .map(|&slot| {
            if landed_slot >= slot {
                landed_slot - slot
            } else {
                0
            }
        })
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("test_vote_parsing=info".parse()?)
                .add_directive("yellowstone_grpc_client=info".parse()?),
        )
        .init();

    // Get configuration
    let endpoint = env::var("SVLM_GRPC_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:10000".to_string());
    let access_token = env::var("SVLM_GRPC_ACCESS_TOKEN").ok();
    let vote_account = env::var("TEST_VOTE_ACCOUNT")
        .unwrap_or_else(|_| "CertusDeBmqN8ZawdkxK5kFGMwBXdudvWHYwtNgNhvLu".to_string());

    info!("Connecting to: {}", endpoint);
    info!("Monitoring vote account: {}", vote_account);

    // Build and connect client
    let mut client_builder = GeyserGrpcClient::build_from_shared(endpoint)?;
    if let Some(token) = access_token {
        client_builder = client_builder.x_token(Some(token))?;
    }

    let mut client = client_builder
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .max_decoding_message_size(1024 * 1024 * 1024)
        .connect()
        .await?;

    // Create subscription
    let (mut subscribe_tx, subscribe_rx) = client.subscribe().await?;

    // Subscribe to vote transactions
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

    subscribe_tx.send(request).await?;
    info!("Waiting for vote transactions...");

    // Process first vote transaction
    let mut stream = subscribe_rx;
    while let Some(update_result) = stream.next().await {
        match update_result {
            Ok(update) => {
                if let Some(UpdateOneof::Transaction(tx_update)) = update.update_oneof {
                    if let Some(tx_info) = &tx_update.transaction {
                        if tx_info.is_vote {
                            let landed_slot = tx_update.slot;
                            let signature = bs58::encode(&tx_info.signature).into_string();
                            
                            info!("\n========== VOTE TRANSACTION DETECTED ==========");
                            info!("Signature: {}", signature);
                            info!("Landed slot: {}", landed_slot);
                            
                            // Extract voted slots
                            match extract_voted_slots(tx_info, landed_slot) {
                                Ok(voted_slots) => {
                                    info!("Voted on slots: {:?}", voted_slots);
                                    
                                    // Calculate latencies
                                    let latencies = calculate_latencies(&voted_slots, landed_slot);
                                    info!("Latencies (slots): {:?}", latencies);
                                    
                                    // Calculate statistics
                                    if !latencies.is_empty() {
                                        let max_latency = latencies.iter().max().copied().unwrap_or(0);
                                        let avg_latency: f64 = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
                                        let min_latency = latencies.iter().min().copied().unwrap_or(0);
                                        
                                        info!("\nLatency Statistics:");
                                        info!("  Max: {} slots", max_latency);
                                        info!("  Avg: {:.2} slots", avg_latency);
                                        info!("  Min: {} slots", min_latency);
                                        
                                        // Convert to approximate milliseconds (assuming ~400ms per slot)
                                        info!("\nApproximate times (at ~400ms/slot):");
                                        info!("  Max: {:.1} seconds", max_latency as f64 * 0.4);
                                        info!("  Avg: {:.1} seconds", avg_latency * 0.4);
                                        info!("  Min: {:.1} seconds", min_latency as f64 * 0.4);
                                    }
                                    
                                    info!("==============================================\n");
                                    
                                    // Exit after processing one transaction
                                    return Ok(());
                                }
                                Err(e) => {
                                    error!("Failed to extract voted slots: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Stream error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

use tracing::warn;