# Subscription Manager Enhancements

## Overview

The subscription manager has been enhanced to subscribe to multiple data streams for comprehensive vote latency monitoring:

1. **Slot Subscription**: Tracks the current slot in real-time
2. **Account Subscription**: Monitors vote account state changes
3. **Transaction Subscription**: Kept as backup/verification

## Key Changes

### 1. Added New Proto Imports

```rust
use yellowstone_grpc_proto::{
    geyser::{
        SubscribeRequest,
        SubscribeRequestFilterTransactions,
        SubscribeRequestFilterSlots,        // NEW
        SubscribeRequestFilterAccounts,      // NEW
        SubscribeUpdate,
        subscribe_update::UpdateOneof,
        CommitmentLevel,
    },
};
```

### 2. Enhanced Subscription Request

The `create_vote_subscription_request_static` method now creates a comprehensive subscription:

```rust
// Slot updates - to track current slot
let slot_filter = SubscribeRequestFilterSlots {
    filter_by_commitment: Some(true),
    interslot_updates: Some(false),
};

// Account updates - to monitor vote state changes
let account_filter = SubscribeRequestFilterAccounts {
    account: vec![vote_pubkey.to_string()],
    owner: vec![],
    filters: vec![],
    nonempty_txn_signature: Some(false),
};

// Transaction updates - as backup
let tx_filter = SubscribeRequestFilterTransactions {
    vote: Some(true),
    failed: Some(false),
    account_include: vec![vote_pubkey.to_string()],
    ..Default::default()
};
```

### 3. Current Slot Tracking

Added a new field to track the current slot:

```rust
pub struct SubscriptionManager {
    // ... existing fields ...
    /// Tracks the current slot from slot updates
    current_slot: Arc<DashMap<String, u64>>,
}
```

With a getter method:

```rust
pub fn get_current_slot(&self) -> Option<u64> {
    self.current_slot.get(&"current".to_string()).map(|entry| *entry.value())
}
```

### 4. Enhanced Update Handling

The `handle_stream_static` method now processes three types of updates:

#### Slot Updates
```rust
UpdateOneof::Slot(slot_update) => {
    // Update the current slot for latency calculations
    current_slot.insert("current".to_string(), slot_update.slot);
}
```

#### Account Updates
```rust
UpdateOneof::Account(account_update) => {
    // Parse vote account data to extract vote state
    if let Some(account_info) = &account_update.account {
        // Parse the account data to extract vote state
        match parse_vote_account_data(
            &account_info.data,
            validator.pubkey,
            validator.vote_account,
            account_update.slot,
        ) {
            Ok(vote_latencies) => {
                // Process extracted vote latencies
            }
            Err(e) => {
                // Handle parsing errors
            }
        }
    }
}
```

#### Transaction Updates (existing)
```rust
UpdateOneof::Transaction(tx_update) => {
    // Process vote transactions as before
}
```

### 5. Vote Account Data Parser

Added a new function to parse vote account data:

```rust
pub fn parse_vote_account_data(
    account_data: &[u8],
    validator_pubkey: Pubkey,
    vote_pubkey: Pubkey,
    current_slot: u64,
) -> Result<Vec<VoteLatency>>
```

This function:
- Deserializes the VoteState from account data
- Extracts recent votes (last 32)
- Calculates latencies based on current slot
- Returns VoteLatency objects for storage

## Benefits

1. **Real-time Slot Tracking**: Always know the current slot for accurate latency calculations
2. **Vote State Monitoring**: Direct access to vote account state changes
3. **Multiple Data Sources**: Cross-verification between transaction and account data
4. **Comprehensive Coverage**: Captures all vote activity, not just transactions

## Testing

Run the test example to verify the multi-stream subscription:

```bash
GRPC_ENDPOINT=your_endpoint GRPC_ACCESS_TOKEN=your_token cargo run --example test_multi_subscription
```

This will show:
- Slot updates with current slot tracking
- Account updates with vote state changes
- Transaction updates for vote transactions
- Calculated latencies for each update type

## Future Improvements

1. Optimize account data parsing for performance
2. Add vote state caching to reduce redundant parsing
3. Implement cross-verification between transaction and account data
4. Add metrics for subscription health monitoring