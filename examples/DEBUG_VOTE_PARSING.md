# Debug Vote Parsing Guide

This directory contains debug programs to help understand and fix the vote slot parsing issue in the Solana Vote Latency Monitor.

## The Problem

The current implementation was setting `voted_on_slots = [landed_slot]`, which always results in 0 latency. We need to extract the actual slots being voted on from the transaction data provided by Yellowstone gRPC.

## Debug Programs

### 1. `debug_vote_transaction.rs`
A comprehensive debug tool that prints ALL fields available in a Yellowstone vote transaction update.

**Usage:**
```bash
# Set your gRPC endpoint and optional auth token
export SVLM_GRPC_ENDPOINT="http://your-grpc-endpoint:10000"
export SVLM_GRPC_ACCESS_TOKEN="your-token"  # Optional

# Optionally set a specific vote account to monitor
export DEBUG_VOTE_ACCOUNT="CertusDeBmqN8ZawdkxK5kFGMwBXdudvWHYwtNgNhvLu"

# Set max transactions to capture (default: 5)
export DEBUG_MAX_TRANSACTIONS=3

# Run the debug program
cargo run --example debug_vote_transaction
```

This will show you:
- All fields in `SubscribeUpdateTransactionInfo`
- Transaction message structure
- Raw instruction data
- Decoded vote instructions with slots
- Transaction metadata

### 2. `test_vote_parsing.rs`
A focused test program that demonstrates proper vote slot extraction.

**Usage:**
```bash
# Set your gRPC endpoint
export SVLM_GRPC_ENDPOINT="http://your-grpc-endpoint:10000"
export SVLM_GRPC_ACCESS_TOKEN="your-token"  # Optional

# Optionally set a vote account
export TEST_VOTE_ACCOUNT="CertusDeBmqN8ZawdkxK5kFGMwBXdudvWHYwtNgNhvLu"

# Run the test
cargo run --example test_vote_parsing
```

This will:
- Connect to Yellowstone gRPC
- Wait for a vote transaction
- Extract voted slots from the transaction data
- Calculate and display latency statistics

## Key Findings

From the Yellowstone transaction structure:
1. **Transaction Data IS Available**: The `SubscribeUpdateTransactionInfo` includes the full transaction with message and instructions
2. **Vote Instructions Can Be Decoded**: The instruction data contains serialized `VoteInstruction` that can be deserialized with bincode
3. **Multiple Slots Per Vote**: Validators typically vote on multiple slots at once (e.g., slots 12340-12345)
4. **Latency Calculation**: `latency = landed_slot - voted_on_slot` for each voted slot

## Implementation Fix

The fix has been applied to `src/modules/parser.rs` in the `parse_yellowstone_vote_transaction` function:

1. Iterate through transaction instructions
2. Find instructions with Vote program ID
3. Deserialize instruction data as `VoteInstruction`
4. Extract slots from Vote, VoteSwitch, UpdateVoteState, or UpdateVoteStateSwitch
5. Calculate latency for each voted slot

## Testing the Fix

After running the main application with the fix:
```bash
cargo run -- --config config/config.toml
```

You should see:
- Non-zero latencies in the logs
- Multiple voted slots per transaction
- Realistic latency values (typically 1-10 slots)

## Common Vote Instruction Types

1. **Vote**: Simple vote with slots array
2. **VoteSwitch**: Vote with proof of switch
3. **UpdateVoteState**: Newer format with lockouts
4. **UpdateVoteStateSwitch**: UpdateVoteState with switch proof

All of these contain slot information that we extract.

## Troubleshooting

If you're still seeing 0 latency:
1. Check that the vote account is actively voting
2. Verify the gRPC endpoint is returning transaction data
3. Look for "Failed to deserialize vote instruction" warnings
4. Ensure the landed_slot > voted_on_slots