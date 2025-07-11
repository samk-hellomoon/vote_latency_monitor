//! Simplified vote monitor to test basic functionality

use futures::{StreamExt, SinkExt};
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;
use yellowstone_grpc_client::{GeyserGrpcClient, ClientTlsConfig};
use yellowstone_grpc_proto::geyser::{
    SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeRequestFilterSlots,
    subscribe_update::UpdateOneof, CommitmentLevel,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    // Read config
    let config_path = "config/mainnet.toml";
    let config = config::Config::builder()
        .add_source(config::File::with_name(config_path))
        .build()?;
    
    let endpoint: String = config.get("grpc.endpoint")?;
    let access_token: String = config.get("grpc.access_token")?;
    
    println!("Connecting to: {}", endpoint);
    
    // Connect to gRPC
    let client_builder = GeyserGrpcClient::build_from_shared(endpoint.clone())
        .map_err(|e| format!("Invalid endpoint: {}", e))?;
    
    let client_builder = if !access_token.trim().is_empty() {
        client_builder.x_token(Some(access_token.trim().to_string()))
            .map_err(|e| format!("Invalid access token: {}", e))?
    } else {
        client_builder
    };
    
    let mut client = client_builder
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(10))
        .tls_config(ClientTlsConfig::new().with_native_roots())
        .map_err(|e| format!("TLS config error: {}", e))?
        .connect()
        .await?;
    
    // Subscribe to slots AND vote transactions
    let (mut subscribe_tx, mut stream) = client.subscribe().await?;
    
    // Create comprehensive subscription
    let mut slots = HashMap::new();
    slots.insert("slots".to_string(), SubscribeRequestFilterSlots {
        filter_by_commitment: Some(true),
        interslot_updates: Some(false),
    });
    
    let mut transactions = HashMap::new();
    transactions.insert("votes".to_string(), SubscribeRequestFilterTransactions {
        vote: Some(true),
        failed: Some(false),
        ..Default::default()
    });
    
    let request = SubscribeRequest {
        slots,
        transactions,
        commitment: Some(CommitmentLevel::Processed as i32),
        from_slot: None,
        ..Default::default()
    };
    
    subscribe_tx.send(request).await?;
    println!("Subscribed to slots and vote transactions");
    
    let mut current_slot = 0u64;
    let mut vote_count = 0u32;
    let mut success_count = 0u32;
    
    while let Some(update) = stream.next().await {
        match update {
            Ok(msg) => {
                match msg.update_oneof {
                    Some(UpdateOneof::Slot(slot_update)) => {
                        current_slot = slot_update.slot;
                        if slot_update.slot % 100 == 0 {
                            println!("Current slot: {}", current_slot);
                        }
                    }
                    Some(UpdateOneof::Transaction(tx)) => {
                        if let Some(tx_info) = &tx.transaction {
                            if tx_info.is_vote {
                                vote_count += 1;
                                
                                // Try to find voted slots in the transaction
                                if let Some(transaction) = &tx_info.transaction {
                                    if let Some(message) = &transaction.message {
                                        println!("\n=== Vote Transaction #{} ===", vote_count);
                                        println!("Landed in slot: {}", tx.slot);
                                        println!("Current slot: {}", current_slot);
                                        println!("Account keys: {}", message.account_keys.len());
                                        println!("Instructions: {}", message.instructions.len());
                                        
                                        // Look for vote program
                                        let vote_program = "Vote111111111111111111111111111111111111111"
                                            .parse::<Pubkey>()
                                            .unwrap();
                                        
                                        for (i, key) in message.account_keys.iter().enumerate() {
                                            if let Ok(pubkey) = Pubkey::try_from(key.as_slice()) {
                                                if pubkey == vote_program {
                                                        println!("Vote program at index: {}", i);
                                                }
                                            }
                                        }
                                        
                                        // Check each instruction
                                        for (idx, inst) in message.instructions.iter().enumerate() {
                                            println!("\nInstruction {}: program_index={}, data_len={}", 
                                                idx, inst.program_id_index, inst.data.len());
                                            
                                            // Print first 10 bytes of instruction data
                                            if !inst.data.is_empty() {
                                                print!("Data bytes: ");
                                                for (_i, byte) in inst.data.iter().take(10).enumerate() {
                                                    print!("{:02x} ", byte);
                                                }
                                                println!("...");
                                            }
                                        }
                                        
                                        success_count += 1;
                                    }
                                }
                                
                                if vote_count % 10 == 0 {
                                    println!("\nProcessed {} votes, {} with data", vote_count, success_count);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}