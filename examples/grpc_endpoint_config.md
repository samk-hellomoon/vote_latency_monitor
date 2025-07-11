# Yellowstone gRPC Endpoint Configuration Examples

The Solana Vote Latency Monitor uses the official Yellowstone gRPC client to subscribe to real-time vote transactions. This document provides configuration examples for various deployment scenarios.

## Overview

The monitor requires access to a Yellowstone gRPC-enabled endpoint to receive real-time vote updates. Without a gRPC endpoint, the system can still run in "discovery-only" mode, which periodically fetches validator information via standard RPC but cannot monitor vote latency in real-time.

## Configuration Options

### 1. Local Validator with Yellowstone Plugin

If you're running a local Solana validator with the Yellowstone gRPC plugin installed:

```toml
[solana]
rpc_endpoint = "http://localhost:8899"

[grpc]
endpoint = "http://localhost:10000"
```

To set up a local validator with Yellowstone:
1. Download the Yellowstone gRPC plugin from [rpcpool/yellowstone-grpc](https://github.com/rpcpool/yellowstone-grpc)
2. Configure the plugin to expose vote account updates
3. Start your validator with `--geyser-plugin-config yellowstone-config.json`

### 2. Hellomoon Yellowstone Service

Hellomoon provides Yellowstone gRPC endpoints for their customers:

```toml
[solana]
rpc_endpoint = "https://your-instance.fleet.hellomoon.io:8899"

[grpc]
endpoint = "https://your-instance.fleet.hellomoon.io:2083"
```

Note: You'll need valid Hellomoon credentials and an active subscription.

### 3. Triton One

Triton provides high-performance Yellowstone gRPC endpoints:

```toml
[solana]
rpc_endpoint = "https://api.mainnet-beta.solana.com"

[grpc]
endpoint = "https://your-endpoint.triton.one:443"
```

### 4. Helius

Helius offers Yellowstone gRPC as part of their infrastructure:

```toml
[solana]
rpc_endpoint = "https://mainnet.helius-rpc.com"

[grpc]
endpoint = "https://mainnet.helius-rpc.com:443"
```

### 5. Discovery-Only Mode (No gRPC)

If you don't have access to a Yellowstone endpoint, you can run in discovery-only mode:

```toml
[solana]
rpc_endpoint = "https://api.mainnet-beta.solana.com"

[grpc]
# Leave endpoint commented out or empty
# endpoint = ""
```

Then run with:
```bash
./svlm run --discovery-only
```

## Authentication

Some Yellowstone providers require authentication. The Yellowstone gRPC client supports several authentication methods:

### Token Authentication
```toml
[grpc]
endpoint = "https://your-endpoint.provider.com:443"
auth_token = "your-auth-token"  # If required by provider
```

### TLS Client Certificates
For providers requiring client certificates, you'll need to configure TLS:
```toml
[grpc]
endpoint = "https://your-endpoint.provider.com:443"
tls_cert_path = "/path/to/client.crt"
tls_key_path = "/path/to/client.key"
tls_ca_cert_path = "/path/to/ca.crt"  # Optional
```

## Environment Variable Overrides

You can override the gRPC endpoint via environment variable:

```bash
export SVLM_GRPC_ENDPOINT="https://new-endpoint.provider.com:443"
./svlm run
```

## Connection Tuning

### For Local Development (Laptop)
```toml
[grpc]
endpoint = "http://localhost:10000"
max_subscriptions = 25  # Start low
buffer_size = 5000  # Smaller buffer
connection_timeout_secs = 30
reconnect_interval_secs = 5
```

### For Production (Dedicated Server)
```toml
[grpc]
endpoint = "https://your-endpoint.provider.com:443"
max_subscriptions = 100  # Can handle more
buffer_size = 50000  # Larger buffer
connection_timeout_secs = 60
reconnect_interval_secs = 10
```

## Troubleshooting

### Common Issues

1. **"Connection refused" errors**
   - Verify the endpoint URL and port
   - Check if Yellowstone plugin is running (for local validators)
   - Ensure you have valid credentials (for remote providers)

2. **"Invalid URI" errors**
   - Use proper URL format: `protocol://host:port`
   - For TLS: use `https://`
   - For non-TLS: use `http://`

3. **Authentication failures**
   - Check your auth token or certificates
   - Verify credentials with your provider
   - Some providers require IP whitelisting

4. **High latency or timeouts**
   - Choose a geographically closer endpoint
   - Reduce `max_subscriptions` to lower load
   - Check your network connection

### Debug Logging

Enable debug logs to see connection details:
```bash
export SVLM_LOG_LEVEL=debug
./svlm run
```

This will show:
- Exact endpoint being used
- Connection attempts and failures
- Authentication details (without sensitive data)
- Subscription status for each validator

## Provider-Specific Notes

### Yellowstone Plugin Versions
- The monitor uses `yellowstone-grpc-client` v8.0.0
- Ensure your Yellowstone plugin is compatible
- Older plugins may require different configuration

### Rate Limits
- Most providers have rate limits
- Start with fewer subscriptions and increase gradually
- Monitor the metrics endpoint for connection errors

### Geographic Considerations
- Choose endpoints close to your location
- Latency affects real-time monitoring accuracy
- Consider running multiple instances in different regions