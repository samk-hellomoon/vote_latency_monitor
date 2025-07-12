//! Vote Parser Module
//!
//! This module is responsible for parsing Solana vote transactions and extracting
//! relevant information such as vote state, slot numbers, and timestamps.

use anyhow::Result;
use async_trait::async_trait;
use solana_sdk::{
    instruction::CompiledInstruction,
    pubkey::Pubkey,
    transaction::Transaction,
    vote::instruction::VoteInstruction,
};
use tracing::{debug, error, trace, warn};
use yellowstone_grpc_proto::prelude::SubscribeUpdateTransactionInfo;

use crate::models::{VoteLatency, VoteTransaction};

/// Parse vote transaction from Yellowstone protobuf format
/// This is a more direct approach that works with the pre-filtered vote transactions
pub fn parse_yellowstone_vote_transaction(
    tx_info: &SubscribeUpdateTransactionInfo,
    validator_pubkey: Pubkey,
    vote_pubkey: Pubkey,
    slot: u64,
) -> Result<VoteLatency> {
    debug!("Parsing Yellowstone vote transaction");
    
    // Extract signature
    let signature = bs58::encode(&tx_info.signature).into_string();
    
    // The slot parameter is the landed slot
    let landed_slot = slot;
    
    // Extract voted slots from the transaction data
    let mut voted_on_slots = Vec::new();
    
    // Check if we have transaction data
    if let Some(tx) = &tx_info.transaction {
        if let Some(message) = &tx.message {
            // Get the vote program ID
            let vote_program_id: Pubkey = VOTE_PROGRAM_ID.parse()?;
            
            // Iterate through instructions
            for (idx, instruction) in message.instructions.iter().enumerate() {
                debug!("Instruction: {:?}", instruction);
                // Get the program ID for this instruction
                if let Some(program_key) = message.account_keys.get(instruction.program_id_index as usize) {
                    let program_pubkey = Pubkey::try_from(program_key.as_slice())
                        .map_err(|e| anyhow::anyhow!("Invalid program pubkey: {}", e))?;
                    
                    // Check if this is a vote program instruction
                    if program_pubkey == vote_program_id {
                        trace!("Found vote program instruction {} with {} bytes of data", idx, instruction.data.len());
                        
                        // Log instruction data info for debugging
                        if instruction.data.len() > 0 {
                            debug!("Vote instruction data: first byte = 0x{:02x}, length = {}", 
                                instruction.data[0], instruction.data.len());
                        }
                        
                        // Try to deserialize the instruction data
                        match bincode::deserialize::<VoteInstruction>(&instruction.data) {
                            Ok(vote_inst) => {
                                match vote_inst {
                                    VoteInstruction::Vote(vote) => {
                                        debug!("Decoded Vote instruction with {} slots", vote.slots.len());
                                        voted_on_slots.extend(&vote.slots);
                                    }
                                    VoteInstruction::VoteSwitch(vote, _) => {
                                        debug!("Decoded VoteSwitch instruction with {} slots", vote.slots.len());
                                        voted_on_slots.extend(&vote.slots);
                                    }
                                    VoteInstruction::UpdateVoteState(update) => {
                                        let slots: Vec<u64> = update.lockouts.iter()
                                            .map(|l| l.slot())
                                            .collect();
                                        debug!("Decoded UpdateVoteState instruction with {} slots", slots.len());
                                        voted_on_slots.extend(&slots);
                                    }
                                    VoteInstruction::UpdateVoteStateSwitch(update, _) => {
                                        let slots: Vec<u64> = update.lockouts.iter()
                                            .map(|l| l.slot())
                                            .collect();
                                        debug!("Decoded UpdateVoteStateSwitch instruction with {} slots", slots.len());
                                        voted_on_slots.extend(&slots);
                                    }
                                    VoteInstruction::TowerSync(tower_sync) => {
                                        // Only take the most recent vote (last lockout)
                                        if let Some(latest_lockout) = tower_sync.lockouts.back() {
                                            let latest_slot = latest_lockout.slot();
                                            debug!("Decoded TowerSync instruction, taking most recent vote: slot {}", latest_slot);
                                            voted_on_slots.push(latest_slot);
                                        } else {
                                            debug!("Decoded TowerSync instruction with no lockouts");
                                        }
                                    }
                                    VoteInstruction::TowerSyncSwitch(tower_sync, _) => {
                                        // Only take the most recent vote (last lockout)
                                        if let Some(latest_lockout) = tower_sync.lockouts.back() {
                                            let latest_slot = latest_lockout.slot();
                                            debug!("Decoded TowerSyncSwitch instruction, taking most recent vote: slot {}", latest_slot);
                                            voted_on_slots.push(latest_slot);
                                        } else {
                                            debug!("Decoded TowerSyncSwitch instruction with no lockouts");
                                        }
                                    }
                                    _ => {
                                        trace!("Vote instruction type does not contain vote data");
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to deserialize vote instruction {}: {}", idx, e);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Remove duplicates and sort
    voted_on_slots.sort_unstable();
    voted_on_slots.dedup();
    
    // If no voted slots found, use landed slot as fallback
    if voted_on_slots.is_empty() {
        warn!("No voted slots found in transaction, using landed slot {} as fallback", landed_slot);
        voted_on_slots.push(landed_slot);
    } else {
        debug!("Extracted {} unique voted slots: {:?}", voted_on_slots.len(), voted_on_slots);
    }
    
    // Use current time as timestamps (approximation for real-time processing)
    let vote_timestamp = chrono::Utc::now();
    let received_timestamp = chrono::Utc::now();
    
    // Find the highest voted slot for backward compatibility
    let highest_voted_slot = voted_on_slots.iter().max().copied().unwrap_or(slot);
    
    // For TowerSync, we only track the most recent vote, so use single-value constructor
    // if we have exactly one voted slot (which is typical for TowerSync)
    if voted_on_slots.len() == 1 {
        Ok(VoteLatency::new_single_vote(
            validator_pubkey,
            vote_pubkey,
            voted_on_slots[0],
            vote_timestamp,
            received_timestamp,
            signature,
            landed_slot,
        ))
    } else {
        // Fall back to multi-slot constructor for other vote types
        Ok(VoteLatency::new_with_slots(
            validator_pubkey,
            vote_pubkey,
            highest_voted_slot,
            vote_timestamp,
            received_timestamp,
            signature,
            voted_on_slots,
            landed_slot,
        ))
    }
}

/// Vote program ID on Solana
pub const VOTE_PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";

/// Trait for vote parsing implementations
#[async_trait]
pub trait VoteParserTrait: Send + Sync {
    /// Parse a vote transaction
    async fn parse(&self, transaction: &VoteTransaction) -> Result<VoteLatency>;
    
    /// Check if a transaction is a vote transaction
    async fn is_vote_transaction(&self, transaction: &Transaction) -> bool;
}

/// Vote parser implementation
/// 
/// This parser is responsible for extracting vote information from Solana transactions.
/// It supports parsing various vote instruction types including Vote, VoteSwitch,
/// UpdateVoteState, and UpdateVoteStateSwitch.
/// 
/// Note: With Yellowstone gRPC, transactions are pre-filtered as vote transactions,
/// simplifying the parsing process.
pub struct VoteParser {
    vote_program_id: Pubkey,
}

impl VoteParser {
    /// Create a new vote parser
    pub fn new() -> Result<Self> {
        Ok(Self {
            vote_program_id: VOTE_PROGRAM_ID.parse()?,
        })
    }
    
    /// Get the latest slot from a list of slots
    pub fn get_latest_slot(slots: &[u64]) -> Option<u64> {
        slots.iter().max().copied()
    }
    
    /// Check if this is a vote transaction by examining program IDs
    pub fn has_vote_program(&self, transaction: &Transaction) -> bool {
        transaction.message.account_keys.contains(&self.vote_program_id)
    }

    /// Parse vote instruction data from raw bytes
    /// This is used when we have access to the raw instruction data
    fn parse_vote_instruction(&self, data: &[u8]) -> Result<VoteInfo> {
        trace!("Parsing vote instruction data: {} bytes", data.len());
        
        // VoteInstruction uses bincode serialization, not borsh
        // Try to deserialize the vote instruction
        match bincode::deserialize::<VoteInstruction>(data) {
            Ok(instruction) => {
                match instruction {
                    VoteInstruction::Vote(vote) => {
                        debug!("Parsed Vote instruction with {} slots", vote.slots.len());
                        Ok(VoteInfo {
                            slots: vote.slots,
                            hash: vote.hash,
                            timestamp: vote.timestamp,
                        })
                    }
                    VoteInstruction::VoteSwitch(vote, _) => {
                        debug!("Parsed VoteSwitch instruction with {} slots", vote.slots.len());
                        Ok(VoteInfo {
                            slots: vote.slots,
                            hash: vote.hash,
                            timestamp: vote.timestamp,
                        })
                    }
                    VoteInstruction::UpdateVoteState(vote_state_update) => {
                        debug!("Parsed UpdateVoteState instruction");
                        // Extract slots from lockouts
                        let slots: Vec<u64> = vote_state_update.lockouts.iter()
                            .map(|lockout| lockout.slot())
                            .collect();
                        Ok(VoteInfo {
                            slots,
                            hash: vote_state_update.hash,
                            timestamp: vote_state_update.timestamp,
                        })
                    }
                    VoteInstruction::UpdateVoteStateSwitch(vote_state_update, _) => {
                        debug!("Parsed UpdateVoteStateSwitch instruction");
                        let slots: Vec<u64> = vote_state_update.lockouts.iter()
                            .map(|lockout| lockout.slot())
                            .collect();
                        Ok(VoteInfo {
                            slots,
                            hash: vote_state_update.hash,
                            timestamp: vote_state_update.timestamp,
                        })
                    }
                    VoteInstruction::TowerSync(tower_sync) => {
                        debug!("Parsed TowerSync instruction with {} lockouts", tower_sync.lockouts.len());
                        // Only take the most recent vote (last lockout)
                        let slots = if let Some(latest_lockout) = tower_sync.lockouts.back() {
                            vec![latest_lockout.slot()]
                        } else {
                            vec![]
                        };
                        Ok(VoteInfo {
                            slots,
                            hash: tower_sync.hash,
                            timestamp: tower_sync.timestamp,
                        })
                    }
                    VoteInstruction::TowerSyncSwitch(tower_sync, _) => {
                        debug!("Parsed TowerSyncSwitch instruction with {} lockouts", tower_sync.lockouts.len());
                        // Only take the most recent vote (last lockout)
                        let slots = if let Some(latest_lockout) = tower_sync.lockouts.back() {
                            vec![latest_lockout.slot()]
                        } else {
                            vec![]
                        };
                        Ok(VoteInfo {
                            slots,
                            hash: tower_sync.hash,
                            timestamp: tower_sync.timestamp,
                        })
                    }
                    _ => {
                        // Other vote instructions don't contain vote data
                        warn!("Vote instruction type does not contain vote data");
                        Ok(VoteInfo {
                            slots: vec![],
                            hash: Default::default(),
                            timestamp: None,
                        })
                    }
                }
            }
            Err(e) => {
                error!("Failed to deserialize vote instruction: {}", e);
                Err(anyhow::anyhow!("Failed to deserialize vote instruction: {}", e))
            }
        }
    }

    /// Extract vote instructions from a transaction
    fn extract_vote_instructions<'a>(
        &self,
        transaction: &'a Transaction,
    ) -> Vec<&'a CompiledInstruction> {
        transaction
            .message
            .instructions
            .iter()
            .filter(|instruction| {
                transaction.message.account_keys
                    .get(instruction.program_id_index as usize)
                    .map(|pubkey| pubkey == &self.vote_program_id)
                    .unwrap_or(false)
            })
            .collect()
    }
    
    /// Extract voted slots from raw transaction data
    /// This deserializes the transaction and extracts vote instructions
    fn extract_voted_slots_from_raw_data(&self, raw_data: &[u8]) -> Result<Vec<u64>> {
        debug!("Extracting voted slots from {} bytes of raw data", raw_data.len());
        
        // Deserialize the transaction from raw bytes
        let transaction: Transaction = match bincode::deserialize(raw_data) {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to deserialize transaction: {}", e);
                return Err(anyhow::anyhow!("Failed to deserialize transaction: {}", e));
            }
        };
        
        // Extract vote instructions
        let vote_instructions = self.extract_vote_instructions(&transaction);
        
        if vote_instructions.is_empty() {
            warn!("No vote instructions found in transaction");
            return Ok(vec![]);
        }
        
        // Collect all voted slots from all vote instructions
        let mut all_slots = Vec::new();
        
        for instruction in vote_instructions {
            trace!("Processing vote instruction with {} bytes of data", instruction.data.len());
            
            // Parse the vote instruction data
            match self.parse_vote_instruction(&instruction.data) {
                Ok(vote_info) => {
                    debug!("Found {} slots in vote instruction", vote_info.slots.len());
                    all_slots.extend(vote_info.slots);
                }
                Err(e) => {
                    warn!("Failed to parse vote instruction: {}", e);
                    // Continue with other instructions
                }
            }
        }
        
        // Remove duplicates and sort
        all_slots.sort_unstable();
        all_slots.dedup();
        
        debug!("Extracted {} unique voted slots", all_slots.len());
        Ok(all_slots)
    }
}

impl Default for VoteParser {
    fn default() -> Self {
        Self::new().expect("Failed to create default VoteParser")
    }
}

#[async_trait]
impl VoteParserTrait for VoteParser {
    async fn parse(&self, vote_tx: &VoteTransaction) -> Result<VoteLatency> {
        debug!("Parsing vote transaction: {}", vote_tx.signature);
        
        // The landed slot is provided in the VoteTransaction
        let landed_slot = vote_tx.landed_slot.unwrap_or(vote_tx.slot);
        
        // Extract voted_on_slots from the transaction data
        let voted_on_slots = if !vote_tx.raw_data.is_empty() {
            match self.extract_voted_slots_from_raw_data(&vote_tx.raw_data) {
                Ok(slots) => {
                    debug!("Extracted {} voted slots from transaction", slots.len());
                    slots
                }
                Err(e) => {
                    warn!("Failed to extract voted slots from raw data: {}", e);
                    // Fall back to using the provided voted_on_slots or a default
                    if !vote_tx.voted_on_slots.is_empty() {
                        vote_tx.voted_on_slots.clone()
                    } else {
                        vec![vote_tx.slot]
                    }
                }
            }
        } else if !vote_tx.voted_on_slots.is_empty() {
            // Use pre-populated voted_on_slots if available
            vote_tx.voted_on_slots.clone()
        } else {
            // Fallback to single slot
            warn!("No vote slot data available, using transaction slot as fallback");
            vec![vote_tx.slot]
        };
        
        // Log debug information
        debug!(
            "Vote transaction parsing - Landed slot: {}, Voted slots: {:?}",
            landed_slot, voted_on_slots
        );
        
        // Calculate slot-based latencies
        let latency_slots: Vec<u8> = voted_on_slots
            .iter()
            .map(|&voted_slot| {
                if landed_slot >= voted_slot {
                    let latency = landed_slot - voted_slot;
                    debug!("Slot {} landed at {}, latency: {} slots", voted_slot, landed_slot, latency);
                    std::cmp::min(latency, 255) as u8
                } else {
                    warn!(
                        "Invalid latency: voted slot {} > landed slot {}",
                        voted_slot, landed_slot
                    );
                    0
                }
            })
            .collect();
        
        // Log latency statistics
        if !latency_slots.is_empty() {
            let max_latency = latency_slots.iter().max().copied().unwrap_or(0);
            let avg_latency: f32 = latency_slots.iter().map(|&x| x as f32).sum::<f32>() 
                / latency_slots.len() as f32;
            debug!(
                "Calculated latencies - Max: {} slots, Avg: {:.1} slots",
                max_latency, avg_latency
            );
        }
        
        // Use the transaction timestamp
        let vote_timestamp = vote_tx.timestamp;
        let received_timestamp = chrono::Utc::now();
        
        // Find the highest voted slot for backward compatibility
        let highest_voted_slot = voted_on_slots.iter().max().copied().unwrap_or(vote_tx.slot);
        
        // Use single-value constructor when we have exactly one voted slot
        if voted_on_slots.len() == 1 {
            Ok(VoteLatency::new_single_vote(
                vote_tx.validator_pubkey.clone(),
                vote_tx.vote_pubkey.clone(),
                voted_on_slots[0],
                vote_timestamp,
                received_timestamp,
                vote_tx.signature.clone(),
                landed_slot,
            ))
        } else {
            Ok(VoteLatency::new_with_slots(
                vote_tx.validator_pubkey.clone(),
                vote_tx.vote_pubkey.clone(),
                highest_voted_slot,
                vote_timestamp,
                received_timestamp,
                vote_tx.signature.clone(),
                voted_on_slots,
                landed_slot,
            ))
        }
    }

    async fn is_vote_transaction(&self, transaction: &Transaction) -> bool {
        transaction
            .message
            .account_keys
            .iter()
            .any(|key| key == &self.vote_program_id)
    }
}

/// Parse vote account data to extract vote state
/// This function is called when we receive account updates for vote accounts
/// NOTE: Account updates do not provide accurate landed slot information,
/// so this is primarily for tracking vote state, not for latency calculation
pub fn parse_vote_account_data(
    account_data: &[u8],
    validator_pubkey: Pubkey,
    _vote_pubkey: Pubkey,
    _account_slot: u64,  // The slot when account was updated - not accurate for latency
) -> Result<Vec<VoteLatency>> {
    use solana_sdk::vote::state::{VoteState, VoteStateVersions};
    
    debug!("Parsing vote account data for validator {} (data length: {} bytes)", 
        validator_pubkey, account_data.len());
    
    // Check minimum data length (4 bytes for version + vote state data)
    if account_data.len() < 4 {
        return Err(anyhow::anyhow!("Vote account data too short: {} bytes", account_data.len()));
    }
    
    // The vote account data format has a 4-byte version prefix
    let version_bytes = &account_data[0..4];
    let version = u32::from_le_bytes([
        version_bytes[0], 
        version_bytes[1], 
        version_bytes[2], 
        version_bytes[3]
    ]);
    
    debug!("Vote account version: {}", version);
    
    // Get the actual vote state data after the version prefix
    let vote_state_data = &account_data[4..];
    
    // Deserialize the vote state using bincode (Solana uses bincode for vote state)
    // Try to deserialize as VoteStateVersions which handles different versions
    let vote_state = match bincode::deserialize::<VoteStateVersions>(vote_state_data) {
        Ok(versions) => {
            // Extract the current VoteState from the versioned enum
            match versions {
                VoteStateVersions::V0_23_5(_state) => {
                    // V0_23_5 is very old and has a different structure
                    // For now, return empty as these are unlikely to be encountered
                    warn!("Encountered old V0_23_5 vote state format, skipping");
                    return Ok(vec![]);
                }
                VoteStateVersions::V1_14_11(state) => {
                    // Convert V1_14_11 to current - this version has similar structure
                    let mut current = VoteState::default();
                    current.node_pubkey = state.node_pubkey;
                    current.authorized_withdrawer = state.authorized_withdrawer;
                    current.commission = state.commission;
                    // Convert the votes - V1_14_11 uses Lockout, current uses LandedVote
                    current.votes = state.votes.into_iter()
                        .map(|lockout| lockout.into())
                        .collect();
                    current.root_slot = state.root_slot;
                    current.authorized_voters = state.authorized_voters;
                    // Note: prior_voters has different tuple structure between versions
                    // For simplicity, we'll leave it as default
                    current.epoch_credits = state.epoch_credits;
                    current.last_timestamp = state.last_timestamp;
                    current
                }
                VoteStateVersions::Current(state) => *state,
            }
        }
        Err(e) => {
            // If that fails, try direct VoteState deserialization
            debug!("Failed to deserialize as VoteStateVersions: {}, trying direct VoteState", e);
            bincode::deserialize::<VoteState>(vote_state_data)
                .map_err(|e| anyhow::anyhow!("Failed to deserialize vote state: {}", e))?
        }
    };
    
    debug!("Vote state has {} votes in tower", vote_state.votes.len());
    
    // Note: We cannot calculate accurate latencies from account data alone
    // because we don't know when the vote transaction actually landed.
    // Account updates happen asynchronously and the slot of the account update
    // is not the same as the slot when the vote transaction was processed.
    
    // For now, we return an empty vector since account-based latency is unreliable
    // In the future, we could use this to track vote state for other purposes
    let vote_latencies = Vec::new();
    
    // Log vote state information for debugging
    let recent_votes: Vec<_> = vote_state.votes
        .iter()
        .rev()
        .take(5)
        .collect();
    
    debug!("Vote account has {} votes in tower, most recent: {:?}", 
        vote_state.votes.len(),
        recent_votes.iter().map(|v| v.slot()).collect::<Vec<_>>()
    );
    
    // Log additional vote state information if available
    debug!("Vote state last timestamp: {:?}", vote_state.last_timestamp);
    
    debug!("Extracted {} vote latencies from account data", vote_latencies.len());
    Ok(vote_latencies)
}

/// Vote instruction information
/// 
/// Contains the parsed data from a vote instruction, including the slots
/// being voted on, the hash, and an optional timestamp.
#[derive(Debug, Clone)]
struct VoteInfo {
    /// Slots being voted on
    slots: Vec<u64>,
    /// Hash of the vote
    hash: solana_sdk::hash::Hash,
    /// Optional timestamp (Unix timestamp in seconds)
    timestamp: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash,
        message::Message,
        signature::Keypair,
        signer::Signer,
        transaction::Transaction,
    };

    #[test]
    fn test_vote_parser_creation() {
        let parser = VoteParser::new().unwrap();
        assert_eq!(
            parser.vote_program_id.to_string(),
            VOTE_PROGRAM_ID
        );
    }

    #[test]
    fn test_get_latest_slot() {
        assert_eq!(VoteParser::get_latest_slot(&[]), None);
        assert_eq!(VoteParser::get_latest_slot(&[100]), Some(100));
        assert_eq!(VoteParser::get_latest_slot(&[100, 200, 150]), Some(200));
        assert_eq!(VoteParser::get_latest_slot(&[300, 200, 250]), Some(300));
    }

    #[tokio::test]
    async fn test_is_vote_transaction() {
        let parser = VoteParser::new().unwrap();
        
        // Create a transaction with vote program
        let vote_program_id = VOTE_PROGRAM_ID.parse().unwrap();
        let payer = Keypair::new();
        let instruction = solana_sdk::system_instruction::transfer(
            &payer.pubkey(),
            &vote_program_id,
            1,
        );
        
        let message = Message::new(&[instruction], Some(&payer.pubkey()));
        let mut transaction = Transaction::new_unsigned(message);
        
        // Add vote program to account keys
        transaction.message.account_keys.push(vote_program_id);
        
        assert!(parser.is_vote_transaction(&transaction).await);
    }

    #[tokio::test]
    async fn test_is_not_vote_transaction() {
        let parser = VoteParser::new().unwrap();
        
        // Create a transaction without vote program
        let payer = Keypair::new();
        let receiver = Keypair::new();
        let instruction = solana_sdk::system_instruction::transfer(
            &payer.pubkey(),
            &receiver.pubkey(),
            1,
        );
        
        let message = Message::new(&[instruction], Some(&payer.pubkey()));
        let transaction = Transaction::new_unsigned(message);
        
        assert!(!parser.is_vote_transaction(&transaction).await);
    }

    #[test]
    fn test_vote_info_struct() {
        let vote_info = VoteInfo {
            slots: vec![100, 101, 102],
            hash: Hash::default(),
            timestamp: Some(1234567890),
        };
        
        assert_eq!(vote_info.slots.len(), 3);
        assert_eq!(vote_info.slots[0], 100);
        assert_eq!(vote_info.timestamp, Some(1234567890));
    }
    
    #[tokio::test]
    async fn test_parse_vote_transaction_with_slots() {
        let parser = VoteParser::new().unwrap();
        
        // Create a test vote transaction
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let timestamp = chrono::Utc::now();
        
        let vote_tx = VoteTransaction {
            signature: "test_sig".to_string(),
            validator_pubkey,
            vote_pubkey,
            slot: 12345,
            timestamp,
            raw_data: vec![], // Empty for this test
            voted_on_slots: vec![12340, 12341, 12342, 12343, 12344, 12345],
            landed_slot: Some(12350),
        };
        
        // Parse the transaction
        let result = parser.parse(&vote_tx).await.unwrap();
        
        // Verify the results
        assert_eq!(result.signature, "test_sig");
        assert_eq!(result.voted_on_slots, vec![12340, 12341, 12342, 12343, 12344, 12345]);
        assert_eq!(result.landed_slot, 12350);
        
        // Check calculated latencies
        assert_eq!(result.latency_slots, vec![10, 9, 8, 7, 6, 5]);
        assert_eq!(result.max_latency_slots(), 10);
        assert_eq!(result.avg_latency_slots(), 7.5);
        
        // Verify slot latency calculation
        assert!(result.verify_slot_latency());
    }
    
    #[tokio::test]
    async fn test_parse_vote_transaction_single_slot() {
        let parser = VoteParser::new().unwrap();
        
        // Create a test vote transaction with single slot (TowerSync-like)
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let timestamp = chrono::Utc::now();
        
        let vote_tx = VoteTransaction {
            signature: "test_sig_single".to_string(),
            validator_pubkey,
            vote_pubkey,
            slot: 12345,
            timestamp,
            raw_data: vec![], // Empty for this test
            voted_on_slots: vec![12345], // Single slot
            landed_slot: Some(12350),
        };
        
        // Parse the transaction
        let result = parser.parse(&vote_tx).await.unwrap();
        
        // Verify the results - should use single-value constructor
        assert_eq!(result.signature, "test_sig_single");
        assert_eq!(result.voted_on_slots, vec![12345]);
        assert_eq!(result.voted_on_slot(), 12345);
        assert_eq!(result.landed_slot, 12350);
        
        // Check calculated latencies
        assert_eq!(result.latency_slots, vec![5]);
        assert_eq!(result.latency_slot(), 5);
        assert_eq!(result.max_latency_slots(), 5);
        assert_eq!(result.avg_latency_slots(), 5.0);
        
        // Verify slot latency calculation
        assert!(result.verify_slot_latency());
    }
    
    #[test]
    fn test_parse_vote_instruction_data() {
        use solana_sdk::vote::state::Vote;
        
        let parser = VoteParser::new().unwrap();
        
        // Create a simple Vote instruction
        let vote = Vote {
            slots: vec![100, 101, 102],
            hash: Hash::default(),
            timestamp: Some(1234567890),
        };
        
        let vote_instruction = VoteInstruction::Vote(vote);
        let data = bincode::serialize(&vote_instruction).unwrap();
        
        // Parse the instruction
        let result = parser.parse_vote_instruction(&data).unwrap();
        
        assert_eq!(result.slots, vec![100, 101, 102]);
        assert_eq!(result.timestamp, Some(1234567890));
    }
    
    #[test]
    fn test_parse_vote_account_data() {
        use solana_sdk::vote::state::{Lockout, VoteStateVersions};
        
        // Create test data
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let account_slot = 1000;
        
        // Create a test vote state with some votes
        let mut vote_state = solana_sdk::vote::state::VoteState::default();
        
        // Add some test votes to the tower
        // In newer versions, votes are LandedVote, not Lockout
        vote_state.votes.push_back(Lockout::new_with_confirmation_count(990, 1).into());
        vote_state.votes.push_back(Lockout::new_with_confirmation_count(995, 2).into());
        vote_state.votes.push_back(Lockout::new_with_confirmation_count(998, 3).into());
        vote_state.votes.push_back(Lockout::new_with_confirmation_count(999, 4).into());
        
        // Wrap in VoteStateVersions
        let vote_state_versions = VoteStateVersions::Current(Box::new(vote_state));
        
        // Serialize the vote state using bincode
        let vote_state_data = bincode::serialize(&vote_state_versions).unwrap();
        
        // Create the full account data with 4-byte version prefix
        let mut account_data = vec![1, 0, 0, 0]; // Version 1
        account_data.extend_from_slice(&vote_state_data);
        
        // Parse the account data
        let result = parse_vote_account_data(
            &account_data,
            validator_pubkey,
            vote_pubkey,
            account_slot
        ).unwrap();
        
        // Verify results - should be empty since we're not calculating latencies from account data
        assert_eq!(result.len(), 0); // No latencies from account data
    }
    
    #[test]
    fn test_parse_vote_account_data_empty() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let account_slot = 1000;
        
        // Test with too short data
        let result = parse_vote_account_data(
            &[1, 2, 3], // Too short
            validator_pubkey,
            vote_pubkey,
            account_slot
        );
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }
}

#[cfg(test)]
#[path = "parser_yellowstone_test.rs"]
mod parser_yellowstone_test;