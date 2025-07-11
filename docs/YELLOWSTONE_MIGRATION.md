# Yellowstone gRPC Client Migration Guide

This document explains the migration from custom gRPC implementation to the official Yellowstone gRPC client for the Solana Vote Latency Monitor.

## Overview

The SVLM project has migrated from a custom gRPC implementation using `tonic` and manually compiled protobuf files to the official `yellowstone-grpc-client` library. This provides better compatibility, maintenance, and support for Yellowstone gRPC endpoints.

## Key Changes

### 1. Dependencies

**Before:**
```toml
[dependencies]
tonic = "0.12"
prost = "0.13"
prost-types = "0.13"

[build-dependencies]
tonic-build = "0.12"
```

**After:**
```toml
[dependencies]
yellowstone-grpc-client = "8.0.0"
yellowstone-grpc-proto = "8.0.0"
tonic = "0.12"  # Still needed for transport layer
```

### 2. Build Process

**Before:**
- Required `build.rs` to compile `.proto` files
- Generated code in `src/grpc/` directory
- Manual protobuf compilation on each build

**After:**
- No custom protobuf compilation needed
- Official client handles all proto definitions
- Cleaner build process

### 3. Client Creation

**Before:**
```rust
use tonic::transport::{Channel, Endpoint};
use crate::grpc::yellowstone_grpc_client::YellowstoneGrpcClient;

let endpoint = Endpoint::from_str(&grpc_url)?
    .timeout(Duration::from_secs(30));
let channel = endpoint.connect().await?;
let client = YellowstoneGrpcClient::new(channel);
```

**After:**
```rust
use yellowstone_grpc_client::{GeyserGrpcClient, ClientTlsConfig};

// For non-TLS
let client = GeyserGrpcClient::builder()
    .endpoint(grpc_url)?
    .build()?;

// For TLS
let tls_config = ClientTlsConfig::builder()
    .with_native_roots()?
    .build()?;

let client = GeyserGrpcClient::builder()
    .endpoint(grpc_url)?
    .tls_config(tls_config)?
    .build()?;
```

### 4. Request/Response Types

**Before:**
```rust
use crate::grpc::{
    SubscribeRequest,
    SubscribeRequestFilterTransactions,
    SubscribeUpdate,
};
```

**After:**
```rust
use yellowstone_grpc_proto::geyser::{
    SubscribeRequest,
    SubscribeRequestFilterTransactions,
    SubscribeUpdate,
    subscribe_update::UpdateOneof,
    CommitmentLevel,
};
```

## Configuration Updates

### gRPC Endpoints

The configuration remains largely the same, but the client now better handles various endpoint formats:

```toml
[grpc]
# Local validator with Yellowstone plugin
endpoint = "http://localhost:10000"

# Remote providers (examples)
# endpoint = "https://your-instance.fleet.hellomoon.io:2083"
# endpoint = "https://your-endpoint.triton.one:443"
```

### TLS Configuration

TLS is now automatically detected based on the URL scheme:
- `https://` endpoints use TLS
- `http://` endpoints do not use TLS

The `enable_tls` configuration field is deprecated and ignored.

## Running Without gRPC

The system now clearly supports a "discovery-only" mode for users without Yellowstone gRPC access:

```bash
# Run without real-time monitoring
./svlm run --discovery-only
```

This mode:
- Periodically discovers validators via RPC
- Does not monitor vote latency in real-time
- Useful for testing or when gRPC is unavailable

## Provider-Specific Notes

### Yellowstone Plugin Versions
- Ensure your Yellowstone plugin is compatible with the client version
- The SVLM uses `yellowstone-grpc-client` v8.0.0
- Check [rpcpool/yellowstone-grpc](https://github.com/rpcpool/yellowstone-grpc) for compatibility

### Authentication
Some providers require authentication tokens:

```bash
# Set via environment variable
export YELLOWSTONE_ACCESS_TOKEN="your-token"
```

Or in code:
```rust
let mut request = Request::new(subscribe_request);
request.metadata_mut().insert("x-token", token.parse()?);
```

## Testing Your Connection

Use the provided examples to test your Yellowstone connection:

```bash
# Basic connection test
cargo run --example test_grpc_connection

# Debug connection issues
YELLOWSTONE_ENDPOINT=https://your-endpoint:443 \
cargo run --example debug_grpc_connection
```

## Troubleshooting

### Common Migration Issues

1. **"Type not found" errors**
   - Update all imports from `crate::grpc::*` to `yellowstone_grpc_proto::geyser::*`
   
2. **Build failures**
   - Remove any `tonic-build` configuration
   - Clean build directory: `cargo clean`
   
3. **Connection errors**
   - Verify endpoint URL format is correct
   - Check TLS requirements for your provider
   
4. **Authentication failures**
   - Update token handling to use proper metadata headers
   - Verify token format with your provider

## Benefits of Migration

1. **Official Support**: Using the official client ensures compatibility
2. **Simplified Build**: No custom protobuf compilation
3. **Better Error Handling**: Improved error messages and diagnostics
4. **Active Maintenance**: Regular updates from the Yellowstone team
5. **Provider Compatibility**: Better support for various Yellowstone providers

## Need Help?

If you encounter issues during migration:
1. Check the [example files](../examples/) for working code
2. Run the debug example to diagnose connection issues
3. Ensure your Yellowstone provider is properly configured
4. Check the official [yellowstone-grpc-client documentation](https://github.com/rpcpool/yellowstone-grpc)