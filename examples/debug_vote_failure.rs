use yellowstone_grpc_client::{GeyserGrpcClient, ClientTlsConfig};
use yellowstone_grpc_proto::geyser::{
    SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeRequestFilterSlots,
    subscribe_update::UpdateOneof, CommitmentLevel,
};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::vote::instruction::VoteInstruction;
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use tokio::time::{timeout, Duration};
use bincode;
use futures::{StreamExt, SinkExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    println!("=== VOTE TRANSACTION DEBUG TOOL ===\n");

    // Connect to Yellowstone
    let endpoint = std::env::var("GRPC_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:10000".to_string());
    let access_token = std::env::var("GRPC_ACCESS_TOKEN").ok()
        .unwrap_or_else(|| "".to_string());
    println!("Connecting to Yellowstone at {}...", endpoint);
    
    // Build client using the new API
    let client_builder = GeyserGrpcClient::build_from_shared(endpoint.to_string())
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
    
    println!("✅ Connected successfully\n");

    // Create subscription for vote transactions
    let vote_program_id = "Vote111111111111111111111111111111111111111";
    println!("Setting up subscription for Vote Program: {}", vote_program_id);
    
    let mut transactions_filter = HashMap::new();
    transactions_filter.insert(
        "vote_txs".to_string(),
        SubscribeRequestFilterTransactions {
            vote: Some(true),
            failed: Some(false),
            signature: None,
            account_include: vec![vote_program_id.to_string()],
            account_exclude: vec![],
            account_required: vec![],
        },
    );

    // Add slots filter to track current slot
    let mut slots_filter = HashMap::new();
    slots_filter.insert(
        "slots".to_string(),
        SubscribeRequestFilterSlots {
            filter_by_commitment: Some(true),
            interslot_updates: Some(false),
        },
    );
    
    let request = SubscribeRequest {
        slots: slots_filter,
        accounts: HashMap::new(),
        transactions: transactions_filter,
        commitment: Some(CommitmentLevel::Processed as i32),
        from_slot: None,
        ..Default::default()
    };

    // Subscribe
    println!("Creating subscription...");
    let (mut subscribe_tx, mut stream) = client.subscribe().await?;
    
    // Send the subscription request
    subscribe_tx.send(request).await?;
    println!("✅ Subscription created\n");

    println!("Waiting for a vote transaction (timeout: 30s)...\n");

    // Wait for one vote transaction with timeout
    let result = timeout(Duration::from_secs(30), async {
        while let Some(message) = stream.next().await {
            match message {
                Ok(msg) => {
                    if let Some(update_oneof) = msg.update_oneof {
                        match update_oneof {
                            UpdateOneof::Transaction(tx_update) => {
                                if let Some(tx_info) = &tx_update.transaction {
                                    if tx_info.is_vote {
                                        println!("🎯 RECEIVED VOTE TRANSACTION!\n");
                                        debug_transaction(tx_info, tx_update.slot);
                                        return Ok::<(), Box<dyn std::error::Error>>(());
                                    }
                                }
                            }
                            UpdateOneof::Slot(slot_update) => {
                                // Optionally track current slot
                                println!("Current slot: {}", slot_update.slot);
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Stream error: {}", e);
                    return Err(Box::new(e));
                }
            }
        }
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Stream ended unexpectedly")))
    }).await;

    match result {
        Ok(Ok(())) => println!("\n✅ Debug completed successfully"),
        Ok(Err(e)) => eprintln!("\n❌ Error during streaming: {}", e),
        Err(_) => eprintln!("\n❌ Timeout: No vote transactions received in 30 seconds"),
    }

    Ok(())
}

fn debug_transaction(tx_info: &yellowstone_grpc_proto::geyser::SubscribeUpdateTransactionInfo, landed_slot: u64) {
    println!("=== TRANSACTION STRUCTURE ===");
    println!("Signature: {}", general_purpose::STANDARD.encode(&tx_info.signature));
    println!("Landed Slot: {}", landed_slot);
    println!("Is Vote: {}", tx_info.is_vote);
    
    if let Some(tx) = &tx_info.transaction {
        println!("\n=== TRANSACTION DATA ===");
        
        if let Some(message) = &tx.message {
            println!("Account keys: {} total", message.account_keys.len());
            println!("Instructions: {} total", message.instructions.len());
            
            // Look for vote program
            let vote_program_id: Pubkey = "Vote111111111111111111111111111111111111111".parse().unwrap();
            
            // Print account keys
            for (i, key) in message.account_keys.iter().enumerate() {
                if let Ok(pubkey) = Pubkey::try_from(key.as_slice()) {
                    println!("Account[{}]: {}", i, pubkey);
                    if pubkey == vote_program_id {
                        println!("   ^ This is the Vote Program!");
                    }
                }
            }
            
            // Analyze instructions
            for (idx, instruction) in message.instructions.iter().enumerate() {
                println!("\nInstruction #{}", idx);
                println!("  Program ID index: {}", instruction.program_id_index);
                
                if let Some(program_key) = message.account_keys.get(instruction.program_id_index as usize) {
                    if let Ok(program_pubkey) = Pubkey::try_from(program_key.as_slice()) {
                        println!("  Program ID: {}", program_pubkey);
                        
                        if program_pubkey == vote_program_id {
                            println!("  🎯 This is a vote instruction!");
                            debug_vote_instruction(&instruction.data);
                            
                            // Try to deserialize as VoteInstruction
                            match bincode::deserialize::<VoteInstruction>(&instruction.data) {
                                Ok(vote_inst) => {
                                    println!("\n✅ Successfully deserialized VoteInstruction!");
                                    match vote_inst {
                                        VoteInstruction::Vote(vote) => {
                                            println!("Type: Vote");
                                            println!("Voted slots: {:?}", vote.slots);
                                            println!("Hash: {:?}", vote.hash);
                                            println!("Timestamp: {:?}", vote.timestamp);
                                        }
                                        VoteInstruction::VoteSwitch(vote, hash) => {
                                            println!("Type: VoteSwitch");
                                            println!("Voted slots: {:?}", vote.slots);
                                            println!("Vote hash: {:?}", vote.hash);
                                            println!("Switch proof hash: {:?}", hash);
                                        }
                                        _ => {
                                            println!("Type: {:?}", vote_inst);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("\n❌ Failed to deserialize as VoteInstruction: {}", e);
                                }
                            }
                        }
                    }
                }
                
                println!("  Account indices: {:?}", instruction.accounts);
                println!("  Data length: {} bytes", instruction.data.len());
            }
        } else {
            println!("No message data available");
        }
    }
    
    if let Some(meta) = &tx_info.meta {
        println!("\n=== TRANSACTION META ===");
        println!("Fee: {} lamports", meta.fee);
        println!("Compute units consumed: {:?}", meta.compute_units_consumed);
        println!("Error: {:?}", meta.err);
        
        if !meta.loaded_writable_addresses.is_empty() {
            println!("\nLoaded writable addresses:");
            for addr in &meta.loaded_writable_addresses {
                println!("  - {}", general_purpose::STANDARD.encode(addr));
            }
        }
        
        if !meta.loaded_readonly_addresses.is_empty() {
            println!("\nLoaded readonly addresses:");
            for addr in &meta.loaded_readonly_addresses {
                println!("  - {}", general_purpose::STANDARD.encode(addr));
            }
        }
    }
}


fn debug_vote_instruction(data: &[u8]) {
    println!("\n=== VOTE INSTRUCTION DATA ===");
    println!("Total length: {} bytes", data.len());
    
    if data.is_empty() {
        println!("❌ Instruction data is empty!");
        return;
    }
    
    // Show first byte (instruction discriminator)
    println!("First byte (discriminator): 0x{:02x} (decimal: {})", data[0], data[0]);
    
    // Show instruction types
    println!("\nKnown Vote instruction types:");
    println!("  0 = InitializeAccount");
    println!("  1 = Authorize");
    println!("  2 = Vote");
    println!("  3 = Withdraw");
    println!("  4 = UpdateValidatorIdentity");
    println!("  5 = UpdateCommission");
    println!("  6 = VoteSwitch");
    println!("  7 = AuthorizeChecked");
    println!("  8 = UpdateVoteState");
    println!("  9 = UpdateVoteStateSwitch");
    println!("  10 = AuthorizeWithSeed");
    println!("  11 = AuthorizeCheckedWithSeed");
    println!("  12 = CompactUpdateVoteState");
    println!("  13 = CompactUpdateVoteStateSwitch");
    println!("  14 = TowerSync");
    println!("  15 = TowerSyncSwitch");
    
    // Hex dump of instruction data
    println!("\nInstruction data (hex):");
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        println!();
    }
    
    // Try to parse based on discriminator
    match data[0] {
        2 | 6 => {
            println!("\n🎯 This looks like a Vote or VoteSwitch instruction!");
            if data.len() > 1 {
                println!("Attempting to parse vote data...");
                
                // Skip discriminator and try to parse
                let vote_data = &data[1..];
                
                // Try compact format first
                match parse_compact_vote(vote_data) {
                    Ok(slots) => {
                        println!("✅ Successfully parsed as COMPACT vote!");
                        println!("Voted slots: {:?}", slots);
                    }
                    Err(e) => {
                        println!("❌ Failed to parse as compact vote: {}", e);
                        
                        // Try legacy format
                        match parse_legacy_vote(vote_data) {
                            Ok((slots, hash, timestamp)) => {
                                println!("✅ Successfully parsed as LEGACY vote!");
                                println!("Voted slots: {:?}", slots);
                                println!("Hash: {:?}", hash);
                                println!("Timestamp: {:?}", timestamp);
                            }
                            Err(e2) => {
                                println!("❌ Failed to parse as legacy vote: {}", e2);
                            }
                        }
                    }
                }
            }
        }
        12 | 13 => {
            println!("\n🎯 This looks like a CompactUpdateVoteState instruction!");
            // These are more complex, just note them for now
        }
        14 | 15 => {
            println!("\n🎯 This looks like a TowerSync instruction!");
            // These are also vote-related
        }
        _ => {
            println!("\n❓ Unknown or non-vote instruction type");
        }
    }
}

fn parse_compact_vote(data: &[u8]) -> Result<Vec<u64>, String> {
    use bincode::Options;
    
    // Compact format: just an array of slots
    let config = bincode::options()
        .with_fixint_encoding()
        .allow_trailing_bytes();
    
    config.deserialize::<Vec<u64>>(data)
        .map_err(|e| format!("Bincode error: {}", e))
}

fn parse_legacy_vote(data: &[u8]) -> Result<(Vec<u64>, Option<[u8; 32]>, Option<i64>), String> {
    use bincode::Options;
    
    // Legacy format: (slots, hash, timestamp)
    let config = bincode::options()
        .with_fixint_encoding()
        .allow_trailing_bytes();
    
    config.deserialize::<(Vec<u64>, Option<[u8; 32]>, Option<i64>)>(data)
        .map_err(|e| format!("Bincode error: {}", e))
}
