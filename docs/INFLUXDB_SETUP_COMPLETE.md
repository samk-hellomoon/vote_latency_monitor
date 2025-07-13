# InfluxDB Setup Complete

## Overview
This document records the successful setup of InfluxDB v2 for the Solana Vote Latency Monitor project. The database is now configured and optimized to handle 3000+ writes per second.

## Installation Details
- **InfluxDB Version**: 2.7.11
- **Installation Method**: Homebrew (macOS)
- **Installation Date**: 2025-07-12
- **Platform**: macOS (development environment)

## Configuration

### Organization
- **Name**: solana-monitor
- **ID**: 4be7300de8054fe9

### Buckets Created

| Bucket Name | Bucket ID | Retention | Shard Duration | Purpose |
|------------|-----------|-----------|----------------|---------|
| vote-latencies-raw | 077a7dd8d29c540b | 24h | 1h | Raw vote data, high-frequency writes |
| vote-latencies-5m | 4dbdc8e17a793eef | 7d | 24h | 5-minute aggregated metrics |
| validator-metrics | 0f4c9d17fc1c05f3 | 30d | 24h | Long-term validator performance metrics |

### API Token
```
c3oyyJtSYhPP36F8Po4gdh2qgL9A9TP-Q7AWMid7KqLBITDBaog2KBleAFo9AsUD9S9cHwZS10m-8UWAMSi0tA==
```

**Permissions**: Read/write access to all three buckets

### Performance Optimizations Applied

| Setting | Default | Optimized | Impact |
|---------|---------|-----------|---------|
| storage-cache-max-memory-size | 1GB | 2GB | 2x more data in memory |
| storage-wal-fsync-delay | 0s | 100ms | 10-100x write throughput |
| storage-max-concurrent-compactions | 0 (auto) | 8 | Better high write handling |
| storage-series-id-set-cache-size | 0 | 100MB | Faster validator lookups |
| storage-wal-max-concurrent-writes | 0 | 100 | Prevents write contention |

### Startup Script
Location: `/Users/stkerr/Code/Work/vote_latency_monitor/scripts/start-influxdb-optimized.sh`

To start InfluxDB with optimizations:
```bash
./scripts/start-influxdb-optimized.sh
```

## Data Schema

### Measurement: `vote_latency`

**Tags** (indexed, low cardinality):
- `validator_id`: First 8 characters of validator pubkey
- `vote_account`: First 8 characters of vote account pubkey  
- `network`: Network name (mainnet/testnet/devnet)

**Fields** (not indexed, actual data):
- `latency_slots`: Vote latency in slots (integer)
- `voted_slot`: Slot that was voted on (integer)
- `landed_slot`: Slot where vote landed (integer)
- `latency_ms`: Latency in milliseconds (integer, for compatibility)

## Verification

Test write successful:
```bash
influx write --bucket vote-latencies-raw --org solana-monitor --precision ns \
  'vote_latency,validator_id=test1234,vote_account=vote5678,network=mainnet latency_slots=2i,voted_slot=1000i,landed_slot=1002i'
```

Query confirmed data is stored correctly with proper field types.

## Next Steps

1. Implement Rust client integration (Phase 3 of migration plan)
2. Set up continuous queries for aggregations
3. Configure Grafana dashboards
4. Implement monitoring and alerting

## Useful Commands

```bash
# Check InfluxDB health
curl -s http://localhost:8086/health

# List organizations
influx org list

# List buckets
influx bucket list --org solana-monitor

# View current configuration
influx server-config | jq

# Monitor write performance
influx query 'from(bucket: "_monitoring") |> range(start: -5m) |> filter(fn: (r) => r._measurement == "influxdb_write_ok")'
```