//! Verify Yellowstone vote parsing is extracting correct slots

use svlm::modules::parser::parse_yellowstone_vote_transaction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::vote::instruction::VoteInstruction;
use solana_vote_interface::state::vote_instruction_data::Vote;
use yellowstone_grpc_proto::prelude::{
    SubscribeUpdateTransactionInfo, Message as ProtoMessage, CompiledInstruction as ProtoInstruction,
    TransactionStatusMeta, Transaction as ProtoTransaction,
};

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create test data
    let validator_pubkey = Pubkey::new_unique();
    let vote_pubkey = Pubkey::new_unique();
    let vote_program_id: Pubkey = "Vote111111111111111111111111111111111111111".parse().unwrap();
    
    // Create a vote for slots 100, 101, 102
    let vote = Vote {
        slots: vec![100, 101, 102],
        hash: solana_sdk::hash::Hash::new_unique(),
        timestamp: None,
    };
    let vote_inst = VoteInstruction::Vote(vote);
    let vote_data = bincode::serialize(&vote_inst).unwrap();
    
    // Create a compiled instruction
    let instruction = ProtoInstruction {
        program_id_index: 0, // Vote program is first in account keys
        accounts: vec![1], // Vote account
        data: vote_data,
    };
    
    // Create the message
    let message = ProtoMessage {
        header: None,
        account_keys: vec![
            vote_program_id.to_bytes().to_vec(), // Index 0: Vote program
            vote_pubkey.to_bytes().to_vec(),     // Index 1: Vote account
        ],
        recent_blockhash: vec![0; 32],
        instructions: vec![instruction],
        versioned: false,
        address_table_lookups: vec![],
    };
    
    // Create the transaction
    let transaction = ProtoTransaction {
        signatures: vec![vec![0; 64]], // Dummy signature
        message: Some(message),
    };
    
    // Create the transaction info
    let tx_info = SubscribeUpdateTransactionInfo {
        signature: vec![0; 64],
        is_vote: true,
        transaction: Some(transaction),
        meta: Some(TransactionStatusMeta::default()),
        index: 0,
    };
    
    // Parse with landed slot 105
    let landed_slot = 105;
    let result = parse_yellowstone_vote_transaction(
        &tx_info,
        validator_pubkey,
        vote_pubkey,
        landed_slot,
    );
    
    match result {
        Ok(vote_latency) => {
            println!("✅ Successfully parsed vote transaction!");
            println!("  Voted on slots: {:?}", vote_latency.voted_on_slots);
            println!("  Landed slot: {}", vote_latency.landed_slot);
            println!("  Latency slots: {:?}", vote_latency.latency_slots);
            println!("  Expected latencies: [5, 4, 3]");
            
            // Verify
            assert_eq!(vote_latency.voted_on_slots, vec![100, 101, 102]);
            assert_eq!(vote_latency.landed_slot, 105);
            assert_eq!(vote_latency.latency_slots, vec![5, 4, 3]);
            println!("\n✅ All assertions passed!");
        }
        Err(e) => {
            eprintln!("❌ Failed to parse: {}", e);
            std::process::exit(1);
        }
    }
}