//! Test program to verify slot-based latency calculation

use svlm::models::{VoteTransaction, VoteLatency};
use svlm::modules::parser::{VoteParser, VoteParserTrait};
use solana_sdk::pubkey::Pubkey;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize parser
    let parser = VoteParser::new()?;
    
    // Create a test vote transaction
    let validator_pubkey = Pubkey::new_unique();
    let vote_pubkey = Pubkey::new_unique();
    
    let mut vote_tx = VoteTransaction {
        signature: "test_signature".to_string(),
        validator_pubkey,
        vote_pubkey,
        slot: 1000,
        timestamp: Utc::now(),
        raw_data: vec![],
        voted_on_slots: vec![995, 996, 997, 998], // Voting on these slots
        landed_slot: Some(1000), // Landing in slot 1000
    };
    
    println!("Test Vote Transaction:");
    println!("  Voted on slots: {:?}", vote_tx.voted_on_slots);
    println!("  Landed slot: {:?}", vote_tx.landed_slot);
    
    // Parse the transaction
    let latency = parser.parse(&vote_tx).await?;
    
    println!("\nCalculated Latencies:");
    println!("  Latency slots: {:?}", latency.latency_slots);
    println!("  Max latency: {} slots", latency.max_latency_slots());
    println!("  Avg latency: {:.1} slots", latency.avg_latency_slots());
    
    // Verify calculations
    for (i, &voted_slot) in vote_tx.voted_on_slots.iter().enumerate() {
        let expected = 1000 - voted_slot;
        let actual = latency.latency_slots[i];
        println!("  Slot {}: expected {} slots, got {} slots", 
                 voted_slot, expected, actual);
    }
    
    Ok(())
}