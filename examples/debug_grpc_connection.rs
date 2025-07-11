//! Debug Yellowstone gRPC connection issues
//!
//! This example helps debug connection issues with Yellowstone gRPC endpoints
//! by testing various configurations and providing detailed error information.

use yellowstone_grpc_client::{GeyserGrpcClient, ClientTlsConfig};
use yellowstone_grpc_proto::geyser::{PingRequest, SubscribeRequest};
use yellowstone_grpc_proto::tonic::{Request, Code};
use tracing::{info, error, debug, warn};
use std::time::Duration;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize detailed tracing
    tracing_subscriber::fmt()
        .with_env_filter("debug,yellowstone_grpc_client=trace")
        .with_target(true)
        .with_thread_ids(true)
        .init();
    
    // Get configuration from environment
    let endpoint_url = env::var("YELLOWSTONE_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:10000".to_string());
    let access_token = env::var("YELLOWSTONE_ACCESS_TOKEN").ok();
    
    info!("=== Yellowstone gRPC Connection Debugging ===");
    info!("Endpoint: {}", endpoint_url);
    info!("Has access token: {}", access_token.is_some());
    
    // Test 1: Basic connection
    info!("\n=== Test 1: Basic Connection ===");
    test_basic_connection(&endpoint_url, &access_token).await;
    
    // Test 2: Connection with different timeout
    info!("\n=== Test 2: Connection with Custom Timeout ===");
    test_connection_with_timeout(&endpoint_url, &access_token, Duration::from_secs(5)).await;
    
    // Test 3: Test subscription
    info!("\n=== Test 3: Test Subscription ===");
    test_subscription(&endpoint_url, &access_token).await;
    
    // Test 4: Connection diagnostics
    info!("\n=== Test 4: Connection Diagnostics ===");
    run_diagnostics(&endpoint_url).await;
    
    Ok(())
}

async fn test_basic_connection(endpoint: &str, access_token: &Option<String>) {
    info!("Testing basic connection to {}", endpoint);
    
    let use_tls = endpoint.starts_with("https://");
    info!("TLS required: {}", use_tls);
    
    let client_result = if use_tls {
        let tls_config = match ClientTlsConfig::builder()
            .with_native_roots() {
            Ok(builder) => match builder.build() {
                Ok(config) => config,
                Err(e) => {
                    error!("Failed to build TLS config: {}", e);
                    return;
                }
            },
            Err(e) => {
                error!("Failed to create TLS builder: {}", e);
                return;
            }
        };
        
        GeyserGrpcClient::builder()
            .endpoint(endpoint)
            .and_then(|b| b.tls_config(tls_config))
            .and_then(|b| b.build())
    } else {
        GeyserGrpcClient::builder()
            .endpoint(endpoint)
            .and_then(|b| b.build())
    };
    
    let client = match client_result {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create client: {}", e);
            return;
        }
    };
    
    info!("Client created, attempting ping...");
    
    let mut request = Request::new(PingRequest {});
    
    // Add access token if provided
    if let Some(token) = access_token {
        match token.parse() {
            Ok(token_value) => {
                request.metadata_mut().insert("x-token", token_value);
                info!("Added access token to request");
            }
            Err(e) => {
                error!("Failed to parse access token: {}", e);
            }
        }
    }
    
    match client.ping(request).await {
        Ok(response) => {
            info!("✓ Ping successful! Response: {:?}", response.into_inner());
        }
        Err(e) => {
            error!("✗ Ping failed: {}", e);
            analyze_error(&e);
        }
    }
}

async fn test_connection_with_timeout(endpoint: &str, access_token: &Option<String>, timeout: Duration) {
    info!("Testing connection with timeout: {:?}", timeout);
    
    let use_tls = endpoint.starts_with("https://");
    
    let client_result = if use_tls {
        let tls_config = match ClientTlsConfig::builder()
            .with_native_roots()
            .and_then(|b| b.build()) {
            Ok(config) => config,
            Err(e) => {
                error!("Failed to build TLS config: {}", e);
                return;
            }
        };
        
        GeyserGrpcClient::builder()
            .endpoint(endpoint)
            .and_then(|b| b.tls_config(tls_config))
            .and_then(|b| b.timeout(timeout))
            .and_then(|b| b.build())
    } else {
        GeyserGrpcClient::builder()
            .endpoint(endpoint)
            .and_then(|b| b.timeout(timeout))
            .and_then(|b| b.build())
    };
    
    let client = match client_result {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create client with timeout: {}", e);
            return;
        }
    };
    
    let mut request = Request::new(PingRequest {});
    if let Some(token) = access_token {
        if let Ok(token_value) = token.parse() {
            request.metadata_mut().insert("x-token", token_value);
        }
    }
    
    match client.ping(request).await {
        Ok(_) => info!("✓ Connection with timeout successful!"),
        Err(e) => {
            error!("✗ Connection with timeout failed: {}", e);
            if e.code() == Code::DeadlineExceeded {
                error!("Timeout was too short or endpoint is slow to respond");
            }
        }
    }
}

async fn test_subscription(endpoint: &str, access_token: &Option<String>) {
    info!("Testing subscription capability...");
    
    let use_tls = endpoint.starts_with("https://");
    
    let client = match create_client(endpoint, use_tls) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create client: {}", e);
            return;
        }
    };
    
    // Create empty subscription request
    let request = SubscribeRequest::default();
    let mut req = Request::new(request);
    
    if let Some(token) = access_token {
        if let Ok(token_value) = token.parse() {
            req.metadata_mut().insert("x-token", token_value);
        }
    }
    
    match client.subscribe(req).await {
        Ok(response) => {
            info!("✓ Subscription request accepted!");
            let mut stream = response.into_inner();
            
            // Try to receive one message
            match tokio::time::timeout(Duration::from_secs(5), stream.message()).await {
                Ok(Ok(Some(msg))) => {
                    info!("✓ Received message from stream!");
                    debug!("Message: {:?}", msg);
                }
                Ok(Ok(None)) => {
                    info!("Stream closed immediately (may need proper filters)");
                }
                Ok(Err(e)) => {
                    error!("Stream error: {}", e);
                }
                Err(_) => {
                    info!("No messages received in 5 seconds (normal for empty subscription)");
                }
            }
        }
        Err(e) => {
            error!("✗ Subscription failed: {}", e);
            analyze_error(&e);
        }
    }
}

async fn run_diagnostics(endpoint: &str) {
    info!("Running connection diagnostics...");
    
    // Parse URL
    match url::Url::parse(endpoint) {
        Ok(url) => {
            info!("✓ Valid URL format");
            info!("  Scheme: {}", url.scheme());
            info!("  Host: {:?}", url.host_str());
            info!("  Port: {:?}", url.port());
            info!("  Path: {}", url.path());
        }
        Err(e) => {
            error!("✗ Invalid URL format: {}", e);
            return;
        }
    }
    
    // Check DNS resolution
    if let Ok(url) = url::Url::parse(endpoint) {
        if let Some(host) = url.host_str() {
            match tokio::net::lookup_host(format!("{}:{}", host, url.port().unwrap_or(443))).await {
                Ok(addrs) => {
                    info!("✓ DNS resolution successful");
                    for addr in addrs {
                        info!("  Resolved to: {}", addr);
                    }
                }
                Err(e) => {
                    error!("✗ DNS resolution failed: {}", e);
                }
            }
        }
    }
    
    // Check port connectivity (basic TCP)
    if let Ok(url) = url::Url::parse(endpoint) {
        if let Some(host) = url.host_str() {
            let port = url.port().unwrap_or(if url.scheme() == "https" { 443 } else { 80 });
            match tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::TcpStream::connect(format!("{}:{}", host, port))
            ).await {
                Ok(Ok(_)) => {
                    info!("✓ TCP connection successful to {}:{}", host, port);
                }
                Ok(Err(e)) => {
                    error!("✗ TCP connection failed: {}", e);
                }
                Err(_) => {
                    error!("✗ TCP connection timeout");
                }
            }
        }
    }
}

fn analyze_error(error: &yellowstone_grpc_proto::tonic::Status) {
    error!("Error Analysis:");
    error!("  Code: {:?}", error.code());
    error!("  Message: {}", error.message());
    
    match error.code() {
        Code::Unavailable => {
            error!("  → The service is unavailable. Possible causes:");
            error!("    - Endpoint URL is incorrect");
            error!("    - Service is down or not running");
            error!("    - Network connectivity issues");
            error!("    - Firewall blocking the connection");
        }
        Code::Unauthenticated => {
            error!("  → Authentication failed. Possible causes:");
            error!("    - Missing or invalid access token");
            error!("    - Token expired");
            error!("    - Incorrect token header name");
        }
        Code::PermissionDenied => {
            error!("  → Permission denied. Possible causes:");
            error!("    - IP not whitelisted");
            error!("    - Insufficient permissions for the operation");
            error!("    - Account quota exceeded");
        }
        Code::InvalidArgument => {
            error!("  → Invalid request. Possible causes:");
            error!("    - Malformed request data");
            error!("    - Unsupported parameters");
        }
        Code::Internal => {
            error!("  → Internal server error. The service encountered an error.");
        }
        _ => {
            error!("  → Unexpected error code");
        }
    }
}

fn create_client(endpoint: &str, use_tls: bool) -> Result<GeyserGrpcClient, Box<dyn std::error::Error>> {
    if use_tls {
        let tls_config = ClientTlsConfig::builder()
            .with_native_roots()?
            .build()?;
        
        Ok(GeyserGrpcClient::builder()
            .endpoint(endpoint)?
            .tls_config(tls_config)?
            .build()?)
    } else {
        Ok(GeyserGrpcClient::builder()
            .endpoint(endpoint)?
            .build()?)
    }
}

// Usage:
// 
// Test local validator:
// cargo run --example debug_grpc_connection
//
// Test remote endpoint with auth:
// YELLOWSTONE_ENDPOINT=https://your-endpoint.provider.com:443 \
// YELLOWSTONE_ACCESS_TOKEN=your-token \
// cargo run --example debug_grpc_connection