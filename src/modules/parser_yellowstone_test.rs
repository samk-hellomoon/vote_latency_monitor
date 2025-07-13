//! Tests for Yellowstone vote transaction parsing

#[cfg(test)]
mod tests {
    use super::super::*;
    use solana_sdk::{
        hash::Hash,
        vote::{
            instruction::VoteInstruction,
            state::Vote,
        },
    };
    use yellowstone_grpc_proto::prelude::{
        Message as ProtoMessage,
        Transaction as ProtoTransaction,
        CompiledInstruction as ProtoInstruction,
        SubscribeUpdateTransactionInfo,
    };

    /// Create a test vote transaction info with specified slots
    fn create_test_vote_tx_info(voted_slots: Vec<u64>) -> SubscribeUpdateTransactionInfo {
        // Create vote instruction
        let vote = Vote {
            slots: voted_slots,
            hash: Hash::default(),
            timestamp: Some(1234567890),
        };
        let vote_instruction = VoteInstruction::Vote(vote);
        let instruction_data = bincode::serialize(&vote_instruction).unwrap();
        
        // Create account keys (vote program at index 0)
        let vote_program_id: Pubkey = VOTE_PROGRAM_ID.parse().unwrap();
        let vote_account = Pubkey::new_unique();
        let validator_account = Pubkey::new_unique();
        
        let account_keys = vec![
            vote_program_id.to_bytes().to_vec(),
            vote_account.to_bytes().to_vec(),
            validator_account.to_bytes().to_vec(),
        ];
        
        // Create instruction pointing to vote program
        let instruction = ProtoInstruction {
            program_id_index: 0, // Points to vote program
            accounts: vec![1, 2], // Vote account and validator
            data: instruction_data,
        };
        
        // Create message
        let message = ProtoMessage {
            header: None,
            account_keys,
            recent_blockhash: Hash::default().to_bytes().to_vec(),
            instructions: vec![instruction],
            address_table_lookups: vec![],
            versioned: false,
        };
        
        // Create transaction
        let transaction = ProtoTransaction {
            signatures: vec![vec![1; 64]], // Dummy signature
            message: Some(message),
        };
        
        // Create transaction info
        SubscribeUpdateTransactionInfo {
            signature: vec![1; 64],
            is_vote: true,
            transaction: Some(transaction),
            meta: None,
            index: 0,
        }
    }

    #[test]
    fn test_parse_yellowstone_vote_transaction_with_multiple_slots() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let landed_slot = 12350;
        let voted_slots = vec![12340, 12342, 12344, 12346, 12348];
        
        let tx_info = create_test_vote_tx_info(voted_slots.clone());
        
        let result = parse_yellowstone_vote_transaction(
            &tx_info,
            validator_pubkey,
            vote_pubkey,
            landed_slot,
        ).unwrap();
        
        // Verify extracted slots
        assert_eq!(result.voted_on_slots, voted_slots);
        assert_eq!(result.landed_slot, landed_slot);
        
        // Verify latencies
        let expected_latencies = vec![10, 8, 6, 4, 2];
        assert_eq!(result.latency_slots, expected_latencies);
        
        // Verify max latency
        assert_eq!(result.max_latency_slots(), 10);
    }

    #[test]
    fn test_parse_yellowstone_vote_transaction_empty_slots() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let landed_slot = 12350;
        
        // Create transaction info with no vote instruction
        let tx_info = SubscribeUpdateTransactionInfo {
            signature: vec![1; 64],
            is_vote: true,
            transaction: None, // No transaction data
            meta: None,
            index: 0,
        };
        
        let result = parse_yellowstone_vote_transaction(
            &tx_info,
            validator_pubkey,
            vote_pubkey,
            landed_slot,
        ).unwrap();
        
        // Should fall back to landed slot
        assert_eq!(result.voted_on_slots, vec![landed_slot]);
        assert_eq!(result.landed_slot, landed_slot);
        assert_eq!(result.latency_slots, vec![0]);
    }

    #[test]
    fn test_parse_yellowstone_vote_transaction_update_vote_state() {
        use solana_sdk::vote::state::{Lockout, VoteStateUpdate};
        
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let landed_slot = 12350;
        
        // Create UpdateVoteState instruction
        let mut lockouts = vec![];
        for slot in [12340, 12342, 12344, 12346, 12348] {
            lockouts.push(Lockout::new(slot));
        }
        
        
        let vote_state_update = VoteStateUpdate {
            lockouts: lockouts.into(),
            root: Some(12330),
            hash: Hash::default(),
            timestamp: Some(1234567890),
        };
        
        let vote_instruction = VoteInstruction::UpdateVoteState(vote_state_update);
        let instruction_data = bincode::serialize(&vote_instruction).unwrap();
        
        // Create transaction info
        let vote_program_id: Pubkey = VOTE_PROGRAM_ID.parse().unwrap();
        let account_keys = vec![
            vote_program_id.to_bytes().to_vec(),
            vote_pubkey.to_bytes().to_vec(),
        ];
        
        let instruction = ProtoInstruction {
            program_id_index: 0,
            accounts: vec![1],
            data: instruction_data,
        };
        
        let message = ProtoMessage {
            header: None,
            account_keys,
            recent_blockhash: Hash::default().to_bytes().to_vec(),
            instructions: vec![instruction],
            address_table_lookups: vec![],
            versioned: false,
        };
        
        let transaction = ProtoTransaction {
            signatures: vec![vec![1; 64]],
            message: Some(message),
        };
        
        let tx_info = SubscribeUpdateTransactionInfo {
            signature: vec![1; 64],
            is_vote: true,
            transaction: Some(transaction),
            meta: None,
            index: 0,
        };
        
        let result = parse_yellowstone_vote_transaction(
            &tx_info,
            validator_pubkey,
            vote_pubkey,
            landed_slot,
        ).unwrap();
        
        // Verify extracted slots from lockouts
        assert_eq!(result.voted_on_slots, vec![12340, 12342, 12344, 12346, 12348]);
        assert_eq!(result.landed_slot, landed_slot);
        assert_eq!(result.max_latency_slots(), 10);
    }
}