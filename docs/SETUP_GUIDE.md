# Solana Vote Latency Monitor - Setup and Deployment Guide

## Prerequisites

### System Requirements
- **Operating System**: Linux, macOS, or Windows (with WSL)
- **RAM**: 4GB minimum (8GB recommended for monitoring many validators)
- **Storage**: 10GB+ available disk space
- **CPU**: 2+ cores recommended

### Software Requirements
1. **Rust 1.70+**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup update stable
   ```

2. **InfluxDB 2.x** (Time-series database for storing metrics)
   ```bash
   # Docker (recommended)
   docker run -d -p 8086:8086 \
     -v influxdb2:/var/lib/influxdb2 \
     -e DOCKER_INFLUXDB_INIT_MODE=setup \
     -e DOCKER_INFLUXDB_INIT_USERNAME=admin \
     -e DOCKER_INFLUXDB_INIT_PASSWORD=adminpassword \
     -e DOCKER_INFLUXDB_INIT_ORG=solana-monitor \
     -e DOCKER_INFLUXDB_INIT_BUCKET=vote-latencies-raw \
     influxdb:2
   
   # Or install directly - see https://docs.influxdata.com/influxdb/v2/install/
   ```

3. **Solana CLI Tools** (optional, for running local validator)
   ```bash
   sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
   ```

## Installation Options

### Option 1: For Local Development (Laptop)

This is the recommended setup for developers running on their personal machines.

#### 1. Clone and Build
```bash
# Clone the repository
git clone https://github.com/your-org/vote_latency_monitor.git
cd vote_latency_monitor

# Build in release mode
cargo build --release

# The binary will be at: ./target/release/svlm
```

#### 2. Configure for Local Use
```bash
# Copy the example configuration
cp config/example.toml config/local.toml

# Edit the configuration
nano config/local.toml
```

Key settings for laptop use:
```toml
[app]
worker_threads = 4  # Limit CPU usage

[solana]
# For local testing with devnet
rpc_endpoint = "https://api.devnet.solana.com"
network = "devnet"

[grpc]
# If you have a Yellowstone gRPC endpoint
endpoint = "https://your-endpoint.provider.com:443"
# Or leave commented to run in discovery-only mode
# endpoint = ""

[storage]
database_path = "./data/svlm_local.db"
retention_days = 3  # Keep data for 3 days only

[discovery]
min_stake_sol = 100.0  # Only monitor validators with 100+ SOL
```

#### 3. Run in Discovery-Only Mode (No gRPC Required)
```bash
# This mode only discovers validators, no real-time monitoring
./target/release/svlm run --discovery-only --config config/local.toml
```

### Option 2: With Yellowstone gRPC Access

If you have access to a Yellowstone gRPC endpoint (local validator or provider):

#### 1. Configure gRPC Endpoint
```toml
[grpc]
# For local validator with Yellowstone plugin
endpoint = "http://localhost:10000"

# For Hellomoon
# endpoint = "https://your-instance.fleet.hellomoon.io:2083"

# For Triton
# endpoint = "https://your-endpoint.triton.one:443"
```

#### 2. Run Full Monitoring
```bash
./target/release/svlm run --config config/local.toml
```

## Yellowstone gRPC Plugin Setup (For Local Validator)

If you want to run your own validator with Yellowstone gRPC:

### 1. Install Yellowstone Plugin
```bash
# Download the latest Yellowstone gRPC plugin
# Check https://github.com/rpcpool/yellowstone-grpc for latest releases
wget https://github.com/rpcpool/yellowstone-grpc/releases/download/v1.x.x/yellowstone-grpc-plugin.so

# Place in your validator's plugin directory
mkdir -p ~/solana-validator/plugins
mv yellowstone-grpc-plugin.so ~/solana-validator/plugins/
```

### 2. Create Yellowstone Configuration
Create `yellowstone-config.json`:
```json
{
  "libpath": "~/solana-validator/plugins/yellowstone-grpc-plugin.so",
  "grpc": {
    "address": "0.0.0.0:10000",
    "channel_capacity": "100000"
  },
  "filters": {
    "accounts": {
      "vote": {
        "account": [],
        "owner": ["Vote111111111111111111111111111111111111111"],
        "filters": []
      }
    }
  }
}
```

### 3. Start Validator with Plugin
```bash
solana-validator \
  --no-voting \
  --geyser-plugin-config yellowstone-config.json \
  --rpc-port 8899 \
  --limit-ledger-size
```

## Database Management

### Initialize Database
```bash
# Create the database with schema
./target/release/svlm init-db --config config/local.toml
```

### View Database Schema
```bash
sqlite3 ./data/svlm_local.db ".schema"
```

### Backup Database
```bash
# Create a backup
sqlite3 ./data/svlm_local.db ".backup ./backups/svlm_backup_$(date +%Y%m%d).db"
```

## Running as a Service (Optional)

For continuous monitoring on a laptop:

### macOS (launchd)
Create `~/Library/LaunchAgents/com.svlm.monitor.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.svlm.monitor</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/svlm</string>
        <string>run</string>
        <string>--config</string>
        <string>/path/to/config/local.toml</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/svlm.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/svlm.error.log</string>
</dict>
</plist>
```

Load the service:
```bash
launchctl load ~/Library/LaunchAgents/com.svlm.monitor.plist
```

### Linux (systemd)
Create `/etc/systemd/system/svlm.service`:
```ini
[Unit]
Description=Solana Vote Latency Monitor
After=network.target

[Service]
Type=simple
User=your-username
WorkingDirectory=/path/to/vote_latency_monitor
ExecStart=/path/to/svlm run --config /path/to/config/local.toml
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable svlm
sudo systemctl start svlm
```

## Monitoring and Maintenance

### View Logs
```bash
# If running in terminal
./target/release/svlm run 2>&1 | tee svlm.log

# With log filtering
SVLM_LOG_LEVEL=debug ./target/release/svlm run
```

### Monitor Resource Usage
```bash
# Check memory and CPU usage
top -p $(pgrep svlm)

# Check database size
du -h ./data/svlm_local.db
```

### Clean Old Data
```bash
# Manual cleanup (keeps last 7 days)
sqlite3 ./data/svlm_local.db "DELETE FROM vote_latencies WHERE timestamp < datetime('now', '-7 days');"

# Vacuum database to reclaim space
sqlite3 ./data/svlm_local.db "VACUUM;"
```

## Troubleshooting

### Common Issues

1. **High CPU Usage**
   - Reduce `worker_threads` in config
   - Lower `max_subscriptions` for gRPC
   - Increase `refresh_interval_secs` for discovery

2. **Database Locked Errors**
   - Ensure only one instance is running
   - Check disk space
   - Enable WAL mode in config

3. **Connection Timeouts**
   - Check firewall settings
   - Verify endpoint URLs
   - Increase timeout values in config

4. **Memory Growth**
   - Enable data retention limits
   - Reduce buffer sizes
   - Monitor with fewer validators

### Debug Mode
```bash
# Enable verbose logging
export SVLM_LOG_LEVEL=debug
export SVLM_APP_DEBUG=true
./target/release/svlm run
```

## Next Steps

1. **Access Metrics**: Visit http://localhost:9090/metrics for Prometheus metrics
2. **Query Data**: Use SQLite CLI or any SQLite browser to analyze collected data
3. **Set Up Alerts**: Configure Prometheus alerting for high latency validators
4. **Optimize Performance**: Adjust configuration based on your system's capabilities