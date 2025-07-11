//! Data models for SVLM
//!
//! This module defines the core data structures used throughout
//! the application for representing validators, votes, and metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Information about a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator identity pubkey
    pub pubkey: Pubkey,
    
    /// Vote account pubkey
    pub vote_account: Pubkey,
    
    /// Validator name (if available)
    pub name: Option<String>,
    
    /// Validator description
    pub description: Option<String>,
    
    /// Validator website
    pub website: Option<String>,
    
    /// gRPC endpoint for subscriptions
    pub grpc_endpoint: Option<String>,
}

/// A vote transaction from a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteTransaction {
    /// Transaction signature
    pub signature: String,
    
    /// Validator identity pubkey
    pub validator_pubkey: Pubkey,
    
    /// Vote account pubkey
    pub vote_pubkey: Pubkey,
    
    /// Slot number being voted on
    /// @deprecated Use voted_on_slots for accurate multi-slot vote tracking
    pub slot: u64,
    
    /// Timestamp when the vote was created
    pub timestamp: DateTime<Utc>,
    
    /// Raw transaction data
    #[serde(skip)]
    pub raw_data: Vec<u8>,
    
    /// All slots being voted on in this transaction (can be multiple)
    #[serde(default)]
    pub voted_on_slots: Vec<u64>,
    
    /// The slot where this vote transaction will land
    #[serde(default)]
    pub landed_slot: Option<u64>,
}

/// Calculated vote latency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteLatency {
    /// Validator identity pubkey
    pub validator_pubkey: Pubkey,
    
    /// Vote account pubkey
    pub vote_pubkey: Pubkey,
    
    /// Slot number
    pub slot: u64,
    
    /// When the vote was created
    pub vote_timestamp: DateTime<Utc>,
    
    /// When we received the vote
    pub received_timestamp: DateTime<Utc>,
    
    /// Calculated latency in milliseconds
    /// @deprecated Use latency_slots instead for accurate slot-based latency
    pub latency_ms: u64,
    
    /// Transaction signature
    pub signature: String,
    
    /// The slots being voted on (can be multiple in a single vote transaction)
    pub voted_on_slots: Vec<u64>,
    
    /// The slot where the vote transaction landed
    pub landed_slot: u64,
    
    /// Latency for each voted slot (landed_slot - voted_on_slot)
    /// Each value represents the latency in slots, capped at 255
    pub latency_slots: Vec<u8>,
}

/// Aggregated latency metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LatencyMetrics {
    /// Mean latency in milliseconds
    /// @deprecated Use mean_slots instead for accurate slot-based metrics
    pub mean_ms: f64,
    
    /// Median latency in milliseconds
    /// @deprecated Use median_slots instead for accurate slot-based metrics
    pub median_ms: f64,
    
    /// 95th percentile latency
    /// @deprecated Use p95_slots instead for accurate slot-based metrics
    pub p95_ms: f64,
    
    /// 99th percentile latency
    /// @deprecated Use p99_slots instead for accurate slot-based metrics
    pub p99_ms: f64,
    
    /// Minimum latency
    /// @deprecated Use min_slots instead for accurate slot-based metrics
    pub min_ms: f64,
    
    /// Maximum latency
    /// @deprecated Use max_slots instead for accurate slot-based metrics
    pub max_ms: f64,
    
    /// Mean latency in slots
    pub mean_slots: f32,
    
    /// Median latency in slots
    pub median_slots: f32,
    
    /// 95th percentile latency in slots
    pub p95_slots: f32,
    
    /// 99th percentile latency in slots
    pub p99_slots: f32,
    
    /// Minimum latency in slots
    pub min_slots: f32,
    
    /// Maximum latency in slots
    pub max_slots: f32,
    
    /// Number of votes with 1 slot latency
    pub votes_1_slot: u64,
    
    /// Number of votes with 2 slots latency
    pub votes_2_slots: u64,
    
    /// Number of votes with 3+ slots latency
    pub votes_3plus_slots: u64,
    
    /// Number of samples
    pub sample_count: u64,
    
    /// Timestamp of calculation
    pub timestamp: DateTime<Utc>,
}

/// Network-wide statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Total number of validators monitored
    pub validator_count: usize,
    
    /// Total votes processed
    pub total_votes: u64,
    
    /// Global latency metrics
    pub global_metrics: LatencyMetrics,
    
    /// Top performing validators
    pub top_validators: Vec<ValidatorPerformance>,
    
    /// Underperforming validators
    pub lagging_validators: Vec<ValidatorPerformance>,
    
    /// Timestamp of calculation
    pub timestamp: DateTime<Utc>,
}

/// Individual validator performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorPerformance {
    /// Validator pubkey
    pub pubkey: Pubkey,
    
    /// Validator name
    pub name: Option<String>,
    
    /// Latency metrics
    pub metrics: LatencyMetrics,
    
    /// Reliability score (0-100)
    pub reliability_score: f64,
    
    /// Number of missed votes
    pub missed_votes: u64,
}

/// Alert for latency anomalies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyAlert {
    /// Alert ID
    pub id: String,
    
    /// Alert type
    pub alert_type: AlertType,
    
    /// Affected validator (if specific to one)
    pub validator_pubkey: Option<Pubkey>,
    
    /// Alert message
    pub message: String,
    
    /// Alert severity
    pub severity: AlertSeverity,
    
    /// When the alert was triggered
    pub triggered_at: DateTime<Utc>,
    
    /// Associated metrics
    pub metrics: Option<LatencyMetrics>,
}

/// Types of alerts
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertType {
    /// High latency detected
    HighLatency,
    
    /// Sudden latency spike
    LatencySpike,
    
    /// Validator connection lost
    ConnectionLost,
    
    /// Network-wide issue
    NetworkAnomaly,
    
    /// Validator delinquent
    ValidatorDelinquent,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    /// Informational
    Info,
    
    /// Warning
    Warning,
    
    /// Critical
    Critical,
}

/// Query parameters for historical data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalQuery {
    /// Start time (inclusive)
    pub start_time: DateTime<Utc>,
    
    /// End time (inclusive)
    pub end_time: DateTime<Utc>,
    
    /// Specific validator (optional)
    pub validator_pubkey: Option<Pubkey>,
    
    /// Aggregation interval (e.g., "1m", "5m", "1h")
    pub interval: Option<String>,
    
    /// Maximum number of results
    pub limit: Option<usize>,
}

/// Response for historical queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalResponse {
    /// Query parameters used
    pub query: HistoricalQuery,
    
    /// Data points
    pub data: Vec<HistoricalDataPoint>,
    
    /// Total records matching query
    pub total_count: usize,
}

/// A single historical data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalDataPoint {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Metrics for this time period
    pub metrics: LatencyMetrics,
    
    /// Validator (if query was for specific validator)
    pub validator_pubkey: Option<Pubkey>,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            validator_count: 0,
            total_votes: 0,
            global_metrics: LatencyMetrics::default(),
            top_validators: vec![],
            lagging_validators: vec![],
            timestamp: Utc::now(),
        }
    }
}

impl ValidatorInfo {
    /// Create a new ValidatorInfo
    pub fn new(pubkey: Pubkey, vote_account: Pubkey) -> Self {
        Self {
            pubkey,
            vote_account,
            name: None,
            description: None,
            website: None,
            grpc_endpoint: None,
        }
    }
}

impl VoteLatency {
    /// Create a new VoteLatency with calculated latency
    /// @deprecated Use new_with_slots for slot-based latency calculation
    pub fn new(
        validator_pubkey: Pubkey,
        vote_pubkey: Pubkey,
        slot: u64,
        vote_timestamp: DateTime<Utc>,
        received_timestamp: DateTime<Utc>,
        signature: String,
    ) -> Self {
        let latency_ms = (received_timestamp - vote_timestamp).num_milliseconds() as u64;
        Self {
            validator_pubkey,
            vote_pubkey,
            slot,
            vote_timestamp,
            received_timestamp,
            latency_ms,
            signature,
            voted_on_slots: vec![slot], // Assume single slot for backward compatibility
            landed_slot: slot, // Assume same slot for backward compatibility
            latency_slots: vec![0], // Zero latency for backward compatibility
        }
    }
    
    /// Create a new VoteLatency with slot-based latency calculation
    pub fn new_with_slots(
        validator_pubkey: Pubkey,
        vote_pubkey: Pubkey,
        slot: u64, // This can be the highest voted slot for backward compatibility
        vote_timestamp: DateTime<Utc>,
        received_timestamp: DateTime<Utc>,
        signature: String,
        voted_on_slots: Vec<u64>,
        landed_slot: u64,
    ) -> Self {
        let latency_ms = (received_timestamp - vote_timestamp).num_milliseconds() as u64;
        
        // Calculate latency for each voted slot
        let latency_slots: Vec<u8> = voted_on_slots
            .iter()
            .map(|&voted_slot| {
                if landed_slot >= voted_slot {
                    let latency = landed_slot - voted_slot;
                    // Cap at 255 slots
                    std::cmp::min(latency, 255) as u8
                } else {
                    // This shouldn't happen in normal operation
                    0
                }
            })
            .collect();
        
        Self {
            validator_pubkey,
            vote_pubkey,
            slot,
            vote_timestamp,
            received_timestamp,
            latency_ms,
            signature,
            voted_on_slots,
            landed_slot,
            latency_slots,
        }
    }
    
    /// Get the maximum latency in slots across all voted slots
    pub fn max_latency_slots(&self) -> u8 {
        self.latency_slots.iter().copied().max().unwrap_or(0)
    }
    
    /// Get the average latency in slots across all voted slots
    pub fn avg_latency_slots(&self) -> f32 {
        if self.latency_slots.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.latency_slots.iter().map(|&x| x as u32).sum();
        sum as f32 / self.latency_slots.len() as f32
    }
    
    /// Verify that the stored latency matches the calculated latency
    pub fn verify_latency(&self) -> bool {
        let calculated = (self.received_timestamp - self.vote_timestamp).num_milliseconds() as u64;
        calculated == self.latency_ms
    }
    
    /// Verify that the slot-based latencies match the expected calculations
    pub fn verify_slot_latency(&self) -> bool {
        if self.voted_on_slots.len() != self.latency_slots.len() {
            return false;
        }
        
        for (i, &voted_slot) in self.voted_on_slots.iter().enumerate() {
            let expected_latency = if self.landed_slot >= voted_slot {
                std::cmp::min(self.landed_slot - voted_slot, 255) as u8
            } else {
                0
            };
            
            if self.latency_slots[i] != expected_latency {
                return false;
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_info_creation() {
        let pubkey = Pubkey::new_unique();
        let vote_account = Pubkey::new_unique();
        
        let info = ValidatorInfo::new(pubkey, vote_account);
        assert_eq!(info.pubkey, pubkey);
        assert_eq!(info.vote_account, vote_account);
        assert!(info.name.is_none());
    }

    #[test]
    fn test_vote_latency_calculation() {
        let vote_time = Utc::now();
        let received_time = vote_time + chrono::Duration::milliseconds(150);
        
        let latency = VoteLatency::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            12345,
            vote_time,
            received_time,
            "test_signature".to_string(),
        );
        
        assert_eq!(latency.latency_ms, 150);
        assert!(latency.verify_latency());
    }
    
    #[test]
    fn test_vote_latency_verify() {
        let vote_time = Utc::now();
        let received_time = vote_time + chrono::Duration::milliseconds(200);
        
        let mut latency = VoteLatency::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            12345,
            vote_time,
            received_time,
            "test_signature".to_string(),
        );
        
        // Should verify correctly
        assert!(latency.verify_latency());
        
        // Corrupt the latency value
        latency.latency_ms = 100;
        assert!(!latency.verify_latency());
    }

    #[test]
    fn test_alert_severity_ordering() {
        assert!(AlertSeverity::Info < AlertSeverity::Warning);
        assert!(AlertSeverity::Warning < AlertSeverity::Critical);
    }
    
    #[test]
    fn test_latency_metrics_default() {
        let metrics = LatencyMetrics::default();
        assert_eq!(metrics.mean_ms, 0.0);
        assert_eq!(metrics.median_ms, 0.0);
        assert_eq!(metrics.sample_count, 0);
        // Test new slot-based fields
        assert_eq!(metrics.mean_slots, 0.0);
        assert_eq!(metrics.median_slots, 0.0);
        assert_eq!(metrics.votes_1_slot, 0);
        assert_eq!(metrics.votes_2_slots, 0);
        assert_eq!(metrics.votes_3plus_slots, 0);
    }
    
    #[test]
    fn test_network_stats_default() {
        let stats = NetworkStats::default();
        assert_eq!(stats.validator_count, 0);
        assert_eq!(stats.total_votes, 0);
        assert!(stats.top_validators.is_empty());
        assert!(stats.lagging_validators.is_empty());
    }
    
    #[test]
    fn test_vote_transaction_creation() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let timestamp = Utc::now();
        
        let vote_tx = VoteTransaction {
            signature: "test_sig".to_string(),
            validator_pubkey,
            vote_pubkey,
            slot: 12345,
            timestamp,
            raw_data: vec![1, 2, 3, 4],
            voted_on_slots: vec![12343, 12344, 12345],
            landed_slot: Some(12350),
        };
        
        assert_eq!(vote_tx.signature, "test_sig");
        assert_eq!(vote_tx.slot, 12345);
        assert_eq!(vote_tx.raw_data.len(), 4);
        assert_eq!(vote_tx.voted_on_slots, vec![12343, 12344, 12345]);
        assert_eq!(vote_tx.landed_slot, Some(12350));
    }
    
    #[test]
    fn test_validator_performance_struct() {
        let pubkey = Pubkey::new_unique();
        let metrics = LatencyMetrics::default();
        
        let perf = ValidatorPerformance {
            pubkey,
            name: Some("Test Validator".to_string()),
            metrics,
            reliability_score: 95.5,
            missed_votes: 10,
        };
        
        assert_eq!(perf.name.as_deref(), Some("Test Validator"));
        assert_eq!(perf.reliability_score, 95.5);
        assert_eq!(perf.missed_votes, 10);
    }
    
    #[test]
    fn test_latency_alert_creation() {
        let alert = LatencyAlert {
            id: "alert-123".to_string(),
            alert_type: AlertType::HighLatency,
            validator_pubkey: Some(Pubkey::new_unique()),
            message: "High latency detected".to_string(),
            severity: AlertSeverity::Warning,
            triggered_at: Utc::now(),
            metrics: None,
        };
        
        assert_eq!(alert.id, "alert-123");
        assert_eq!(alert.alert_type, AlertType::HighLatency);
        assert_eq!(alert.severity, AlertSeverity::Warning);
    }
    
    #[test]
    fn test_historical_query_validation() {
        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now();
        
        let query = HistoricalQuery {
            start_time: start,
            end_time: end,
            validator_pubkey: None,
            interval: Some("5m".to_string()),
            limit: Some(100),
        };
        
        assert!(query.end_time > query.start_time);
        assert_eq!(query.interval.as_deref(), Some("5m"));
        assert_eq!(query.limit, Some(100));
    }
    
    #[test]
    fn test_vote_latency_with_slots() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let vote_time = Utc::now();
        let received_time = vote_time + chrono::Duration::milliseconds(150);
        
        // Test with multiple voted slots
        let voted_on_slots = vec![1000, 1001, 1002];
        let landed_slot = 1005;
        
        let latency = VoteLatency::new_with_slots(
            validator_pubkey,
            vote_pubkey,
            1002, // highest voted slot for backward compatibility
            vote_time,
            received_time,
            "test_sig".to_string(),
            voted_on_slots.clone(),
            landed_slot,
        );
        
        // Check slot-based calculations
        assert_eq!(latency.voted_on_slots, voted_on_slots);
        assert_eq!(latency.landed_slot, landed_slot);
        assert_eq!(latency.latency_slots, vec![5, 4, 3]); // landed_slot - each voted_slot
        assert_eq!(latency.max_latency_slots(), 5);
        assert_eq!(latency.avg_latency_slots(), 4.0);
        assert!(latency.verify_slot_latency());
    }
    
    #[test]
    fn test_vote_latency_slot_capping() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let vote_time = Utc::now();
        let received_time = vote_time + chrono::Duration::milliseconds(150);
        
        // Test with very large slot difference (should cap at 255)
        let voted_on_slots = vec![1000];
        let landed_slot = 1300; // 300 slots difference
        
        let latency = VoteLatency::new_with_slots(
            validator_pubkey,
            vote_pubkey,
            1000,
            vote_time,
            received_time,
            "test_sig".to_string(),
            voted_on_slots,
            landed_slot,
        );
        
        // Should be capped at 255
        assert_eq!(latency.latency_slots, vec![255]);
        assert_eq!(latency.max_latency_slots(), 255);
    }
    
    #[test]
    fn test_vote_latency_backward_compatibility() {
        let validator_pubkey = Pubkey::new_unique();
        let vote_pubkey = Pubkey::new_unique();
        let vote_time = Utc::now();
        let received_time = vote_time + chrono::Duration::milliseconds(150);
        
        // Test old constructor for backward compatibility
        let latency = VoteLatency::new(
            validator_pubkey,
            vote_pubkey,
            1000,
            vote_time,
            received_time,
            "test_sig".to_string(),
        );
        
        // Should have default slot values
        assert_eq!(latency.voted_on_slots, vec![1000]);
        assert_eq!(latency.landed_slot, 1000);
        assert_eq!(latency.latency_slots, vec![0]);
        assert_eq!(latency.latency_ms, 150);
    }
}