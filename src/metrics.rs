//! Metrics collection module for SVLM
//!
//! This module provides Prometheus metrics collection and export
//! for monitoring the health and performance of the system.

use anyhow::Result;
use once_cell::sync::Lazy;
use prometheus::{
    register_gauge_vec, register_histogram_vec, register_int_counter_vec,
    register_int_gauge_vec, Encoder, GaugeVec, HistogramVec, IntCounterVec,
    IntGaugeVec, TextEncoder,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info};
use warp::Filter;

use crate::config::Config;

/// Vote latency histogram buckets (in milliseconds)
const LATENCY_BUCKETS: &[f64] = &[
    10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
];

/// Global metrics registry
pub static METRICS: Lazy<Metrics> = Lazy::new(|| {
    Metrics::new().expect("Failed to initialize metrics")
});

/// Metrics collection structure
pub struct Metrics {
    /// Total votes processed
    pub votes_total: IntCounterVec,
    
    /// Vote processing errors
    pub vote_errors: IntCounterVec,
    
    /// Vote latency histogram
    pub vote_latency: HistogramVec,
    
    /// Active gRPC subscriptions
    pub active_subscriptions: IntGaugeVec,
    
    /// Total validators monitored
    pub validators_total: IntGaugeVec,
    
    /// RPC request counter
    pub rpc_requests: IntCounterVec,
    
    /// RPC request errors
    pub rpc_errors: IntCounterVec,
    
    /// Database operations
    pub db_operations: IntCounterVec,
    
    /// Database errors
    pub db_errors: IntCounterVec,
    
    /// System health gauge (0 = unhealthy, 1 = healthy)
    pub system_health: IntGaugeVec,
    
    /// Current memory usage
    pub memory_usage: GaugeVec,
    
    /// Current CPU usage
    pub cpu_usage: GaugeVec,
}

impl Metrics {
    /// Create new metrics instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            votes_total: register_int_counter_vec!(
                "svlm_votes_total",
                "Total number of votes processed",
                &["validator", "status"]
            )?,
            
            vote_errors: register_int_counter_vec!(
                "svlm_vote_errors_total",
                "Total number of vote processing errors",
                &["validator", "error_type"]
            )?,
            
            vote_latency: register_histogram_vec!(
                "svlm_vote_latency_ms",
                "Vote latency in milliseconds",
                &["validator"],
                LATENCY_BUCKETS.to_vec()
            )?,
            
            active_subscriptions: register_int_gauge_vec!(
                "svlm_active_subscriptions",
                "Number of active gRPC subscriptions",
                &["status"]
            )?,
            
            validators_total: register_int_gauge_vec!(
                "svlm_validators_total",
                "Total number of validators being monitored",
                &["status"]
            )?,
            
            rpc_requests: register_int_counter_vec!(
                "svlm_rpc_requests_total",
                "Total number of RPC requests",
                &["method", "status"]
            )?,
            
            rpc_errors: register_int_counter_vec!(
                "svlm_rpc_errors_total",
                "Total number of RPC errors",
                &["method", "error_type"]
            )?,
            
            db_operations: register_int_counter_vec!(
                "svlm_db_operations_total",
                "Total number of database operations",
                &["operation", "status"]
            )?,
            
            db_errors: register_int_counter_vec!(
                "svlm_db_errors_total",
                "Total number of database errors",
                &["operation", "error_type"]
            )?,
            
            system_health: register_int_gauge_vec!(
                "svlm_system_health",
                "System health status (0 = unhealthy, 1 = healthy)",
                &["component"]
            )?,
            
            memory_usage: register_gauge_vec!(
                "svlm_memory_usage_bytes",
                "Current memory usage in bytes",
                &["type"]
            )?,
            
            cpu_usage: register_gauge_vec!(
                "svlm_cpu_usage_percent",
                "Current CPU usage percentage",
                &["core"]
            )?,
        })
    }

    /// Record a processed vote
    pub fn record_vote(&self, validator: &str, latency_ms: f64) {
        self.votes_total
            .with_label_values(&[validator, "success"])
            .inc();
        
        self.vote_latency
            .with_label_values(&[validator])
            .observe(latency_ms);
    }

    /// Record a vote processing error
    pub fn record_vote_error(&self, validator: &str, error_type: &str) {
        self.vote_errors
            .with_label_values(&[validator, error_type])
            .inc();
        
        self.votes_total
            .with_label_values(&[validator, "error"])
            .inc();
    }

    /// Update active subscriptions count
    pub fn set_active_subscriptions(&self, count: i64) {
        self.active_subscriptions
            .with_label_values(&["active"])
            .set(count);
    }

    /// Update total validators count
    pub fn set_validators_total(&self, active: i64, inactive: i64) {
        self.validators_total
            .with_label_values(&["active"])
            .set(active);
        
        self.validators_total
            .with_label_values(&["inactive"])
            .set(inactive);
    }

    /// Record an RPC request
    pub fn record_rpc_request(&self, method: &str, success: bool) {
        let status = if success { "success" } else { "error" };
        self.rpc_requests
            .with_label_values(&[method, status])
            .inc();
    }

    /// Record an RPC error
    pub fn record_rpc_error(&self, method: &str, error_type: &str) {
        self.rpc_errors
            .with_label_values(&[method, error_type])
            .inc();
    }

    /// Record a database operation
    pub fn record_db_operation(&self, operation: &str, success: bool) {
        let status = if success { "success" } else { "error" };
        self.db_operations
            .with_label_values(&[operation, status])
            .inc();
    }

    /// Record a database error
    pub fn record_db_error(&self, operation: &str, error_type: &str) {
        self.db_errors
            .with_label_values(&[operation, error_type])
            .inc();
    }

    /// Update system health status
    pub fn set_system_health(&self, component: &str, healthy: bool) {
        let value = if healthy { 1 } else { 0 };
        self.system_health
            .with_label_values(&[component])
            .set(value);
    }

    /// Update memory usage
    pub fn set_memory_usage(&self, usage_type: &str, bytes: f64) {
        self.memory_usage
            .with_label_values(&[usage_type])
            .set(bytes);
    }

    /// Update CPU usage
    pub fn set_cpu_usage(&self, core: &str, percent: f64) {
        self.cpu_usage
            .with_label_values(&[core])
            .set(percent);
    }
}

/// Metrics server for Prometheus scraping
pub struct MetricsServer {
    config: Arc<Config>,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Start the metrics HTTP server
    pub async fn start(&self) -> Result<()> {
        if !self.config.metrics.enabled {
            info!("Metrics collection disabled");
            return Ok(());
        }

        let addr: SocketAddr = format!(
            "{}:{}",
            self.config.metrics.bind_address,
            self.config.metrics.port
        )
        .parse()?;

        info!("Starting metrics server on {}", addr);

        // Metrics endpoint
        let metrics_route = warp::path("metrics")
            .and(warp::get())
            .map(|| {
                let encoder = TextEncoder::new();
                let metric_families = prometheus::gather();
                let mut buffer = Vec::new();
                
                match encoder.encode(&metric_families, &mut buffer) {
                    Ok(_) => warp::reply::with_header(
                        buffer,
                        "Content-Type",
                        encoder.format_type(),
                    ),
                    Err(e) => {
                        error!("Failed to encode metrics: {}", e);
                        warp::reply::with_header(
                            Vec::new(),
                            "Content-Type",
                            "text/plain",
                        )
                    }
                }
            });

        // Health check endpoint
        let health_route = warp::path("health")
            .and(warp::get())
            .map(|| warp::reply::json(&serde_json::json!({"status": "ok"})));

        let routes = metrics_route.or(health_route);

        tokio::spawn(async move {
            warp::serve(routes).run(addr).await;
        });

        Ok(())
    }
}

/// Helper function to record errors with proper categorization
pub fn record_error(error: &crate::error::Error) {
    let category = error.category();
    
    match error {
        crate::error::Error::Rpc(_msg) => {
            METRICS.record_rpc_error("unknown", category);
        }
        crate::error::Error::Database(_) => {
            METRICS.record_db_error("unknown", category);
        }
        crate::error::Error::InvalidVote(_) => {
            METRICS.record_vote_error("unknown", category);
        }
        _ => {
            // Other errors can be tracked separately if needed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new().unwrap();
        
        // Record some test metrics
        metrics.record_vote("test_validator", 100.0);
        metrics.set_active_subscriptions(5);
        metrics.set_validators_total(10, 2);
        metrics.set_system_health("database", true);
    }

    #[test]
    fn test_record_vote() {
        METRICS.record_vote("validator1", 150.0);
        METRICS.record_vote_error("validator2", "parse_error");
    }
}