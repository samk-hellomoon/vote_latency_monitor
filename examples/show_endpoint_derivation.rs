//! Show how endpoint URLs are derived

use svlm::config::Config;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Arc::new(Config::load("config/config.toml")?);
    
    println!("=== Endpoint Derivation Logic ===");
    println!("Config RPC endpoint: {}", config.solana.rpc_endpoint);
    println!("Config gRPC endpoint: {:?}", config.grpc.endpoint);
    println!("Config enable_tls: {}", config.grpc.enable_tls);
    println!();
    
    // Test different RPC endpoints
    let test_endpoints = vec![
        "https://api.mainnet-beta.solana.com",
        "https://elite-shield.fleet.hellomoon.io:2083",
        //"http://localhost:8899",
        //"https://example.com:8900",
    ];
    
    for rpc_endpoint in test_endpoints {
        println!("RPC endpoint: {}", rpc_endpoint);
        
        if let Ok(url) = url::Url::parse(rpc_endpoint) {
            let host = url.host_str().unwrap_or("localhost");
            let scheme = url.scheme();
            
            if url.port().is_some() && url.port() != Some(443) && url.port() != Some(80) {
                // Keep the existing URL as-is, preserving the scheme (http/https)
                let path = url.path();
                let path = if path == "/" { "" } else { path };
                let grpc_endpoint = format!("{}://{}:{}{}", 
                    scheme,
                    host, 
                    url.port().unwrap(),
                    path);
                println!("  -> Derived gRPC endpoint: {}", grpc_endpoint);
                println!("  -> Using TLS: {}", scheme == "https" || (scheme == "http" && config.grpc.enable_tls));
            } else {
                // Standard RPC endpoint - add default gRPC port
                let grpc_scheme = if config.grpc.enable_tls { "https" } else { "http" };
                let grpc_endpoint = format!("{}://{}:10000", grpc_scheme, host);
                println!("  -> Derived gRPC endpoint: {}", grpc_endpoint);
                println!("  -> Using TLS: {}", grpc_scheme == "https");
            }
        }
        println!();
    }
    
    Ok(())
}