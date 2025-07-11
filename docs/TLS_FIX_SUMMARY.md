# gRPC TLS Connection Fix Summary

## Problem
The gRPC connection was failing with a transport error when trying to connect to `https://elite-shield.fleet.hellomoon.io:2083` because:

1. The code was not configuring TLS when connecting to HTTPS endpoints
2. The endpoint derivation logic was converting HTTPS URLs to HTTP
3. The `enable_tls` configuration value was not being used when creating connections

## Solution
The following changes were made to fix the issue:

### 1. Added TLS Support to Dependencies
Updated `Cargo.toml` to include the TLS feature for tonic:
```toml
tonic = { version = "0.12", features = ["tls"] }
```

### 2. Updated Subscription Manager
Modified `src/modules/subscription.rs`:

- Added import for `ClientTlsConfig`
- Added logic to detect when TLS is needed based on URL scheme
- Configure TLS on the endpoint when connecting to HTTPS URLs
- Fixed endpoint URL derivation to preserve the original scheme (https://)

### 3. Key Code Changes

#### Connection Logic
```rust
// Parse the URL to determine if TLS is needed
let url = url::Url::parse(endpoint_url)
    .map_err(|e| crate::error::Error::internal(format!("Invalid endpoint URL: {}", e)))?;

let use_tls = url.scheme() == "https" || (url.scheme() == "http" && config.grpc.enable_tls);

let mut endpoint = Endpoint::from_str(endpoint_url)
    .map_err(|e| crate::error::Error::internal(format!("Invalid endpoint: {}", e)))?
    .timeout(std::time::Duration::from_secs(config.grpc.connection_timeout_secs));

// Configure TLS if needed
if use_tls {
    let tls_config = ClientTlsConfig::new();
    endpoint = endpoint.tls_config(tls_config)
        .map_err(|e| crate::error::Error::internal(format!("Failed to configure TLS: {}", e)))?;
}
```

#### URL Derivation Fix
```rust
// Keep the existing URL as-is, preserving the scheme (http/https)
format!("{}://{}:{}{}", 
    scheme,  // Now preserves original scheme instead of forcing http
    host, 
    url.port().unwrap(),
    path)
```

## Testing
Created test utilities in:
- `src/bin/test_tls.rs` - Simple TLS connection test
- `examples/test_grpc_connection.rs` - Full gRPC connection test with ping
- `examples/show_endpoint_derivation.rs` - Shows how endpoints are derived

## Configuration
The `config/config.toml` already has `enable_tls = true` which will now be properly respected.

## Expected Behavior After Fix
1. When connecting to `https://` endpoints, TLS will be automatically configured
2. The original URL scheme will be preserved
3. The `enable_tls` config option will force TLS even for `http://` endpoints when set to true
4. Connection to `https://elite-shield.fleet.hellomoon.io:2083` should now succeed with proper TLS handshake