//! Test Yellowstone gRPC connection
//! 
//! This example demonstrates how to test connectivity to a Yellowstone gRPC endpoint
//! using the official yellowstone-grpc-client library.

use yellowstone_grpc_client::{GeyserGrpcClient, ClientTlsConfig};
use yellowstone_grpc_proto::geyser::{PingRequest};
use yellowstone_grpc_proto::tonic::Request;
use tracing::{info, error};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();
    
    // Get endpoint from environment or use default
    let endpoint = env::var("YELLOWSTONE_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:10000".to_string());
    
    info!("Testing connection to Yellowstone gRPC endpoint: {}", endpoint);
    
    // Parse endpoint to determine if TLS is needed
    let use_tls = endpoint.starts_with("https://");
    info!("TLS enabled: {}", use_tls);
    
    // Create client with optional TLS
    let client = if use_tls {
        let tls_config = ClientTlsConfig::builder()
            .with_native_roots()?
            .build()?;
        
        GeyserGrpcClient::builder()
            .endpoint(endpoint)?
            .tls_config(tls_config)?
            .build()?
    } else {
        GeyserGrpcClient::builder()
            .endpoint(endpoint)?
            .build()?
    };
    
    info!("Client created, attempting to connect...");
    
    // Test with ping
    match client.ping(Request::new(PingRequest {})).await {
        Ok(response) => {
            info!("✓ Ping successful! Response: {:?}", response.into_inner());
            info!("Connection to Yellowstone gRPC is working!");
        }
        Err(e) => {
            error!("✗ Ping failed: {}", e);
            error!("Status code: {:?}", e.code());
            error!("Details: {}", e.message());
            
            // Common error explanations
            match e.code() {
                yellowstone_grpc_proto::tonic::Code::Unavailable => {
                    error!("The endpoint is not reachable. Check:");
                    error!("- Is the Yellowstone plugin running?");
                    error!("- Is the endpoint URL correct?");
                    error!("- Are there any firewall rules blocking the connection?");
                }
                yellowstone_grpc_proto::tonic::Code::Unauthenticated => {
                    error!("Authentication failed. You may need to provide credentials.");
                    error!("Set YELLOWSTONE_ACCESS_TOKEN environment variable if required.");
                }
                yellowstone_grpc_proto::tonic::Code::PermissionDenied => {
                    error!("Permission denied. Check your access credentials or IP whitelist.");
                }
                _ => {
                    error!("Unexpected error. Please check the endpoint configuration.");
                }
            }
            
            return Err(e.into());
        }
    }
    
    // Test subscription capabilities
    info!("\nTesting subscription capabilities...");
    test_subscription(&client).await?;
    
    Ok(())
}

async fn test_subscription(client: &GeyserGrpcClient) -> Result<(), Box<dyn std::error::Error>> {
    use yellowstone_grpc_proto::geyser::{
        SubscribeRequest,
        SubscribeRequestFilterAccounts,
        SubscribeRequestAccountsFilter,
    };
    use futures::StreamExt;
    use std::collections::HashMap;
    
    // Create a simple subscription request for vote accounts
    let mut accounts_filter = HashMap::new();
    accounts_filter.insert(
        "vote_accounts".to_string(),
        SubscribeRequestAccountsFilter {
            // Subscribe to all vote accounts
            owner: vec!["Vote111111111111111111111111111111111111111".to_string()],
            ..Default::default()
        }
    );
    
    let request = SubscribeRequest {
        accounts: SubscribeRequestFilterAccounts {
            filter: accounts_filter,
        },
        ..Default::default()
    };
    
    info!("Creating subscription for vote accounts...");
    
    match client.subscribe(Request::new(request)).await {
        Ok(response) => {
            let mut stream = response.into_inner();
            info!("✓ Subscription created successfully!");
            info!("Waiting for first update (this may take a moment)...");
            
            // Try to get one update
            let timeout = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                stream.next()
            ).await;
            
            match timeout {
                Ok(Some(Ok(update))) => {
                    info!("✓ Received update! Type: {:?}", 
                        update.update_oneof.as_ref().map(|u| std::mem::discriminant(u)));
                    info!("Subscription is working correctly!");
                }
                Ok(Some(Err(e))) => {
                    error!("✗ Stream error: {}", e);
                }
                Ok(None) => {
                    info!("Stream ended (this is normal for a test)");
                }
                Err(_) => {
                    info!("No updates received in 10 seconds (this may be normal if no vote activity)");
                }
            }
        }
        Err(e) => {
            error!("✗ Failed to create subscription: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

// Usage examples:
// 
// Test local validator:
// cargo run --example test_grpc_connection
//
// Test remote endpoint:
// YELLOWSTONE_ENDPOINT=https://your-endpoint.provider.com:443 cargo run --example test_grpc_connection
//
// Test with authentication:
// YELLOWSTONE_ENDPOINT=https://your-endpoint.provider.com:443 \
// YELLOWSTONE_ACCESS_TOKEN=your-token \
// cargo run --example test_grpc_connection