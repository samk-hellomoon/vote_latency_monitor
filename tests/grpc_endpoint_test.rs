use std::sync::Arc;
use svlm::config::Config;
use svlm::modules::subscription::SubscriptionManager;

#[tokio::test]
async fn test_hellomoon_grpc_endpoint_parsing() {
    // Ensure env var is not set from other tests
    std::env::remove_var("SVLM_GRPC_ENDPOINT");
    
    // Test with the Hellomoon endpoint that was causing issues
    let mut config = Config::default();
    config.solana.rpc_endpoint = "https://elite-shield.fleet.hellomoon.io:2083".to_string();
    
    let config = Arc::new(config);
    let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
    
    let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
    
    // The gRPC endpoint should preserve the custom port and path
    assert_eq!(
        manager.grpc_endpoint(), 
        "https://elite-shield.fleet.hellomoon.io:2083"
    );
}

#[tokio::test]
async fn test_explicit_grpc_endpoint() {
    // Ensure env var is not set from other tests
    std::env::remove_var("SVLM_GRPC_ENDPOINT");
    
    // Test with explicit gRPC endpoint in config
    let mut config = Config::default();
    config.solana.rpc_endpoint = "https://api.mainnet-beta.solana.com".to_string();
    config.grpc.endpoint = Some("https://elite-shield.fleet.hellomoon.io:2083".to_string());
    
    let config = Arc::new(config);
    let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
    
    let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
    
    // Should use the explicit gRPC endpoint
    assert_eq!(
        manager.grpc_endpoint(), 
        "https://elite-shield.fleet.hellomoon.io:2083"
    );
}

#[tokio::test]
async fn test_env_var_override() {
    // Clean up first
    std::env::remove_var("SVLM_GRPC_ENDPOINT");
    
    // Set environment variable before creating manager
    std::env::set_var("SVLM_GRPC_ENDPOINT", "http://custom-grpc:9999");
    
    let config = Arc::new(Config::default());
    let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
    
    let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
    
    // Should use the environment variable
    assert_eq!(manager.grpc_endpoint(), "http://custom-grpc:9999");
    
    // Clean up
    std::env::remove_var("SVLM_GRPC_ENDPOINT");
}