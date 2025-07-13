# Solana Vote Latency Monitor (SVLM)

A high-performance monitoring tool that tracks vote latency metrics for Solana validators in real-time using the official Yellowstone gRPC client. Successfully parses TowerSync vote transactions and computes accurate vote latencies by extracting voted slots from transaction instruction data. Designed to run on a trusted developer's laptop for monitoring validator performance.

## Key Achievement

✅ **TowerSync Vote Parsing**: Successfully implemented parsing of TowerSync vote transactions (instruction types 14 & 15) to extract voted slots and calculate accurate vote latencies. The system correctly deserializes compact vote data from transaction instructions and computes `landed_slot - voted_slot` latencies for real-time validator performance monitoring.

## Prerequisites

- **Rust 1.70+** - Install from [rustup.rs](https://rustup.rs/)
- **Solana RPC Access** - Either:
  - Local Solana validator with Yellowstone gRPC plugin enabled (recommended)
  - Access to a Yellowstone gRPC-enabled endpoint (e.g., Hellomoon, Triton)
  - Public RPC endpoint (discovery only, no real-time monitoring)
- **InfluxDB 2.x** - High-performance time-series database
- **4GB+ RAM** - For monitoring validators
- **10GB+ disk space** - For data storage (configurable)

## Quick Start

### 1. Clone and Build

```bash
# Clone the repository
git clone <repository-url>
cd vote_latency_monitor

# Build the project
cargo build --release
```

### 2. Configure

```bash
# Copy the example configuration
cp config/example.toml config/config.toml

# Edit configuration (see Configuration section below)
nano config/config.toml
```

### 3. Run the Monitor

```bash
# Start monitoring (uses config/config.toml by default)
./target/release/svlm run

# Or specify a custom config
./target/release/svlm run --config path/to/config.toml
```

## Configuration Overview

Key configuration settings in `config/config.toml`:

```toml
[solana]
# For local validator (recommended for full functionality)
rpc_endpoint = "http://localhost:8899"

# For public endpoints (discovery only, no gRPC)
# rpc_endpoint = "https://api.mainnet-beta.solana.com"

[grpc]
# Yellowstone gRPC endpoint
# For local validator with Yellowstone plugin:
endpoint = "http://localhost:10000"
# For Hellomoon: "https://YOUR-ENDPOINT.fleet.hellomoon.io:2083"
# For Triton: "https://YOUR-ENDPOINT.triton.one:443"
max_subscriptions = 50  # Start low, increase based on performance

[influxdb]
url = "http://localhost:8086"
org = "solana-monitor"
token = "your-influxdb-token"
bucket = "vote-latencies-raw"
batch_size = 5000
flush_interval_ms = 100

[metrics]
# Prometheus metrics endpoint
enabled = true
bind_address = "127.0.0.1"
port = 9090
```

### Environment Variable Overrides

Any config value can be overridden with environment variables:

```bash
# Override RPC endpoint
export SVLM_SOLANA_RPC_ENDPOINT="https://api.devnet.solana.com"

# Override log level
export SVLM_LOG_LEVEL="debug"

# Override InfluxDB settings
export SVLM_INFLUXDB_URL="http://localhost:8086"
export SVLM_INFLUXDB_TOKEN="your-token"
```

## Basic Usage Examples

### Running with a Local Validator

```bash
# Start a local test validator with Yellowstone gRPC plugin
# You'll need the Yellowstone plugin installed and configured
solana-test-validator \
  --geyser-plugin-config path/to/yellowstone-config.json \
  --rpc-port 8899 \
  --log

# In another terminal, run SVLM
./target/release/svlm run
```

### Running with Public RPC (Limited Mode)

```bash
# Set RPC endpoint and disable gRPC
export SVLM_SOLANA_RPC_ENDPOINT="https://api.mainnet-beta.solana.com"
export SVLM_GRPC_ENABLED="false"

# Run in discovery-only mode
./target/release/svlm run --discovery-only
```

### List Discovered Validators

```bash
# Show all discovered validators
./target/release/svlm list-validators

# Show top validators by stake
./target/release/svlm list-validators --top 20
```

## Querying Collected Data

### Using InfluxDB CLI

```bash
# View recent vote latencies (Flux query)
influx query '
from(bucket: "vote-latencies-raw")
  |> range(start: -1h)
  |> filter(fn: (r) => r._measurement == "vote_latency")
  |> filter(fn: (r) => r._field == "latency_slots")
  |> group(columns: ["validator_pubkey"])
  |> mean()
  |> sort(columns: ["_value"], desc: true)
  |> limit(n: 20)
'

# View validator performance over time
influx query '
from(bucket: "vote-latencies-raw")
  |> range(start: -24h)
  |> filter(fn: (r) => r._measurement == "vote_latency")
  |> filter(fn: (r) => r.validator_pubkey == "YOUR_VALIDATOR_PUBKEY")
  |> filter(fn: (r) => r._field == "latency_slots")
  |> aggregateWindow(every: 1h, fn: mean, createEmpty: false)
'
```

### Export Data to CSV

```bash
# Export hourly latency statistics
influx query -o csv '
from(bucket: "vote-latencies-raw")
  |> range(start: -24h)
  |> filter(fn: (r) => r._measurement == "vote_latency")
  |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
' > latencies_24h.csv
```

### Using the Metrics Endpoint

```bash
# View Prometheus metrics
curl http://localhost:9090/metrics

# Key metrics:
# - svlm_vote_latency_histogram - Latency distribution
# - svlm_active_validators - Number of monitored validators
# - svlm_votes_processed_total - Total votes processed
# - svlm_grpc_connection_errors - Connection error count
```

## Troubleshooting

### Common Issues

#### 1. "Connection refused" on gRPC endpoint

**Problem**: Cannot connect to localhost:10000

**Solution**:
- Ensure your Solana validator has the Yellowstone gRPC plugin installed and enabled
- Check the Yellowstone config includes vote account subscriptions
- Verify the port is correct (default is 10000 for local validators)
- For remote endpoints, ensure you have proper authentication credentials

#### 2. "Rate limit exceeded" errors

**Problem**: Too many RPC requests

**Solution**:
- Reduce `max_concurrent_requests` in config
- Use a local RPC endpoint instead of public
- Increase `refresh_interval_secs` for discovery

#### 3. High memory usage

**Problem**: Memory usage grows over time

**Solution**:
- Reduce `max_subscriptions` to monitor fewer validators
- Decrease `buffer_size` in gRPC config
- Enable more aggressive data retention (`retention_days`)

#### 4. InfluxDB connection errors

**Problem**: Cannot connect to InfluxDB

**Solution**:
- Ensure InfluxDB is running: `docker ps | grep influx`
- Verify URL and port are correct (default: 8086)
- Check token has write permissions to the bucket

#### 5. No validators discovered

**Problem**: Empty validator list

**Solution**:
- Check RPC endpoint connectivity
- Verify network setting matches RPC endpoint
- Try the discovery test: `cargo run --example test_discovery`
- Note: gRPC is not required for validator discovery, only for real-time monitoring

### Debug Mode

Enable detailed logging for troubleshooting:

```bash
# Set debug logging
export SVLM_LOG_LEVEL="debug"
export SVLM_APP_DEBUG="true"

# Run with verbose output
./target/release/svlm run
```

### Performance Tuning

For laptop/limited resources:

```toml
[app]
worker_threads = 4  # Limit CPU usage

[grpc]
max_subscriptions = 25  # Start small
buffer_size = 5000  # Reduce memory usage

[influxdb]
batch_size = 1000  # Smaller batches for limited resources
flush_interval_ms = 500  # Less frequent flushes
num_workers = 2  # Fewer worker threads

[discovery]
refresh_interval_secs = 300  # Less frequent updates
```

## Architecture Notes

### Yellowstone gRPC Client

This project uses the official `yellowstone-grpc-client` library (v8.0.0) for connecting to Yellowstone gRPC endpoints. This provides:

- Native support for all major Yellowstone providers (Hellomoon, Triton, Helius, etc.)
- Automatic TLS handling based on endpoint URL
- Built-in protobuf definitions - no custom compilation needed
- Better error handling and diagnostics

For migration from older versions or custom implementations, see [docs/YELLOWSTONE_MIGRATION.md](docs/YELLOWSTONE_MIGRATION.md).

## Next Steps

- Monitor the Prometheus metrics at http://localhost:9090/metrics
- Query the InfluxDB database for historical analysis
- Adjust configuration based on your system's performance
- Consider setting up Grafana for visualization (see docs/)