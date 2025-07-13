use svlm::config::{Config, DiscoveryConfig, AppConfig, SolanaConfig, GrpcConfig, InfluxConfig, MetricsConfig, LatencyConfig};

fn main() {
    println!("Demonstrating whitelist filtering that accepts both identity and vote account pubkeys\n");
    
    // Example pubkeys
    let identity_pubkey = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2";
    let vote_account = "HMV14UAuULSwqmZhsKHzaVkYAd94iWpEeURgbUegfQLc";
    
    // Create a config with the vote account in the whitelist
    let config = Config {
        discovery: DiscoveryConfig {
            enabled: true,
            refresh_interval_secs: 60,
            min_stake_sol: 0.0,
            include_delinquent: false,
            whitelist: vec![vote_account.to_string()],
            blacklist: vec![],
        },
        // ... other config fields would be here in real usage
        app: AppConfig {
            name: "demo".to_string(),
            log_level: "info".to_string(),
            worker_threads: Some(4),
            debug: false,
        },
        solana: SolanaConfig {
            rpc_endpoint: "http://localhost:8899".to_string(),
            network: "devnet".to_string(),
            timeout_secs: 30,
            max_concurrent_requests: 5,
        },
        grpc: GrpcConfig {
            endpoint: None,
            access_token: None,
            max_subscriptions: 50,
            connection_timeout_secs: 30,
            reconnect_interval_secs: 5,
            buffer_size: 10000,
            enable_tls: false,
        },
        influxdb: InfluxConfig {
            url: "http://localhost:8086".to_string(),
            org: "demo".to_string(),
            token: "demo-token".to_string(),
            bucket: "demo-bucket".to_string(),
            batch_size: 1000,
            flush_interval_ms: 100,
            num_workers: 2,
            enable_compression: false,
        },
        metrics: MetricsConfig {
            enabled: false,
            bind_address: "127.0.0.1".to_string(),
            port: 9090,
            collection_interval_secs: 60,
        },
        latency: LatencyConfig {
            window_size: 100,
            calculate_global_stats: true,
            stats_interval_secs: 30,
            outlier_threshold: 3.0,
        },
    };
    
    // Demonstrate the filtering logic
    println!("Config whitelist contains: {:?}", config.discovery.whitelist);
    println!("Identity pubkey: {}", identity_pubkey);
    println!("Vote account pubkey: {}", vote_account);
    
    // The new filtering logic in discovery.rs will check both:
    let in_whitelist = config.discovery.whitelist.contains(&identity_pubkey.to_string()) 
        || config.discovery.whitelist.contains(&vote_account.to_string());
    
    println!("\nWith the new filtering logic:");
    println!("- Identity pubkey in whitelist: {}", config.discovery.whitelist.contains(&identity_pubkey.to_string()));
    println!("- Vote account pubkey in whitelist: {}", config.discovery.whitelist.contains(&vote_account.to_string()));
    println!("- Validator would be included: {}", in_whitelist);
    
    println!("\nThis validator would now be included even though only the vote account pubkey is in the whitelist!");
}