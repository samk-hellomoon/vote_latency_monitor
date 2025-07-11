# Slot-Based Vote Latency Metrics

## Overview

The Solana Vote Latency Monitor now supports slot-based latency metrics in addition to time-based metrics. This provides a more accurate representation of vote propagation delays in the Solana network.

## What are Slot-Based Metrics?

In Solana, validators vote on blocks (slots) as they are produced. A vote transaction can include votes for multiple slots, and the latency is measured as the difference between:
- **Voted-on slot**: The slot number being voted on
- **Landed slot**: The slot where the vote transaction is included in the blockchain

The formula is: `latency_slots = landed_slot - voted_on_slot`

## Database Schema Updates

### vote_latencies table
New columns added:
- `voted_on_slots TEXT` - JSON array of slot numbers being voted on
- `landed_slot BIGINT` - The slot where the vote transaction landed
- `latency_slots TEXT` - JSON array of latencies (one per voted slot)

### metrics table
New columns added:
- `mean_slots REAL` - Average latency in slots
- `median_slots REAL` - Median latency in slots
- `p95_slots REAL` - 95th percentile latency in slots
- `p99_slots REAL` - 99th percentile latency in slots
- `min_slots REAL` - Minimum latency in slots
- `max_slots REAL` - Maximum latency in slots
- `votes_1_slot INTEGER` - Count of votes with 1 slot latency
- `votes_2_slots INTEGER` - Count of votes with 2 slots latency
- `votes_3plus_slots INTEGER` - Count of votes with 3+ slots latency

## Migration

For existing databases, the system will automatically:
1. Add the new columns to existing tables
2. Create necessary indexes
3. Backfill data for existing records (using conservative defaults)

## Metrics Calculation

The calculator module now tracks both time-based and slot-based metrics:

1. **Slot Latency Distribution**: Tracks how many votes land with 1, 2, or 3+ slots of latency
2. **Statistical Measures**: Calculates mean, median, p95, p99, min, and max for slot-based latencies
3. **Per-Validator Tracking**: Maintains separate metrics for each validator

## Benefits

1. **Network-Agnostic**: Slot-based metrics are not affected by clock synchronization issues
2. **More Accurate**: Directly measures blockchain propagation delay
3. **Better Insights**: Can identify validators that consistently vote late
4. **Trend Analysis**: Can track network-wide vote propagation patterns

## Example Output

```
Global metrics - Mean: 150.23ms (2.1 slots), Median: 145.00ms (2.0 slots), P95: 210.50ms (3.0 slots), Validators: 1234
Vote distribution - 1 slot: 2341, 2 slots: 5678, 3+ slots: 1234
```

## Backward Compatibility

- All existing time-based metrics continue to work
- Old data is preserved and can be queried
- APIs maintain backward compatibility
- Migration is automatic and non-destructive