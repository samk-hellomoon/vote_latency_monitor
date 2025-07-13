//! Storage Manager Module
//!
//! This module defines the storage trait interface for vote latency data.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_sdk::pubkey::Pubkey;

use crate::error::Result;
use crate::models::{LatencyMetrics, ValidatorInfo, VoteLatency};

/// Trait for storage implementations
#[async_trait]
pub trait StorageManagerTrait: Send + Sync {
    /// Initialize the storage backend
    async fn initialize(&self) -> Result<()>;
    
    /// Store a vote latency record
    async fn store_vote_latency(&self, latency: &VoteLatency) -> Result<()>;
    
    /// Store aggregated metrics
    async fn store_metrics(
        &self,
        metrics: &LatencyMetrics,
        validator_pubkey: Option<&Pubkey>,
    ) -> Result<()>;
    
    /// Query vote latencies for a time range
    async fn query_latencies(
        &self,
        validator_pubkey: Option<&Pubkey>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<VoteLatency>>;
    
    /// Get validator information
    async fn get_validator_info(&self, pubkey: &Pubkey) -> Result<Option<ValidatorInfo>>;
    
    /// Store validator information
    async fn store_validator_info(&self, info: &ValidatorInfo) -> Result<()>;
}