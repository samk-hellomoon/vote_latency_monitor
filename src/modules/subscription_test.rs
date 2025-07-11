#[cfg(test)]
mod endpoint_tests {
    use super::super::*;
    use std::sync::Arc;

    fn create_test_config(rpc_endpoint: &str, grpc_endpoint: Option<&str>) -> Arc<Config> {
        let mut config = Config::default();
        config.solana.rpc_endpoint = rpc_endpoint.to_string();
        if let Some(endpoint) = grpc_endpoint {
            config.grpc.endpoint = Some(endpoint.to_string());
        }
        Arc::new(config)
    }

    #[tokio::test]
    async fn test_grpc_endpoint_from_env() {
        // Set environment variable
        std::env::set_var("SVLM_GRPC_ENDPOINT", "http://custom:9999");
        
        let config = create_test_config("https://api.mainnet-beta.solana.com", None);
        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
        
        let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
        assert_eq!(manager.grpc_endpoint, "http://custom:9999");
        
        // Clean up
        std::env::remove_var("SVLM_GRPC_ENDPOINT");
    }

    #[tokio::test]
    async fn test_grpc_endpoint_from_config() {
        // Ensure env var is not set
        std::env::remove_var("SVLM_GRPC_ENDPOINT");
        
        let config = create_test_config(
            "https://api.mainnet-beta.solana.com", 
            Some("http://configured:8888")
        );
        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
        
        let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
        assert_eq!(manager.grpc_endpoint, "http://configured:8888");
    }

    #[tokio::test]
    async fn test_grpc_endpoint_derived_standard() {
        // Ensure env var is not set
        std::env::remove_var("SVLM_GRPC_ENDPOINT");
        
        let config = create_test_config("https://api.mainnet-beta.solana.com", None);
        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
        
        let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
        assert_eq!(manager.grpc_endpoint, "http://api.mainnet-beta.solana.com:10000");
    }

    #[tokio::test]
    async fn test_grpc_endpoint_with_custom_port() {
        // Ensure env var is not set
        std::env::remove_var("SVLM_GRPC_ENDPOINT");
        
        let config = create_test_config("https://elite-shield.fleet.hellomoon.io:2083", None);
        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
        
        let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
        assert_eq!(manager.grpc_endpoint, "http://elite-shield.fleet.hellomoon.io:2083");
    }

    #[tokio::test]
    async fn test_grpc_endpoint_with_path() {
        // Ensure env var is not set
        std::env::remove_var("SVLM_GRPC_ENDPOINT");
        
        let config = create_test_config("https://api.example.com:8080/api/v1", None);
        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
        
        let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
        assert_eq!(manager.grpc_endpoint, "http://api.example.com:8080/api/v1");
    }

    #[tokio::test]
    async fn test_grpc_endpoint_http_to_http() {
        // Ensure env var is not set
        std::env::remove_var("SVLM_GRPC_ENDPOINT");
        
        let config = create_test_config("http://localhost:8899", None);
        let (_, shutdown_rx) = tokio::sync::broadcast::channel(1);
        
        let manager = SubscriptionManager::new(config, shutdown_rx).await.unwrap();
        assert_eq!(manager.grpc_endpoint, "http://localhost:10000");
    }
}