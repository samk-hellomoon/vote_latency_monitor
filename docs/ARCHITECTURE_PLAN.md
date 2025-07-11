# Solana Vote Latency Monitor - Architecture Plan

## Executive Summary

This document presents the comprehensive architectural design for the Solana Vote Latency Monitor (SVLM), a high-performance system designed to track and record vote latency metrics for all active voting validators on the Solana blockchain. The architecture prioritizes performance, scalability, and reliability while maintaining simplicity and minimizing dependencies.

## 1. System Architecture Overview

### 1.1 High-Level Component Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           External Systems                               │
├─────────────────────────────────────────────────────────────────────────┤
│  Solana RPC Node (JSON-RPC)  │  Solana gRPC Endpoint  │  Local Storage │
└───────────┬───────────────────┴────────────┬───────────┴────────────────┘
            │                                │
┌───────────┴───────────────────────────────┴─────────────────────────────┐
│                        SVLM Core System                                  │
├──────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐    │
│  │   Discovery     │    │   Subscription   │    │    Storage      │    │
│  │    Module       │───▶│     Manager      │───▶│    Manager      │    │
│  │                 │    │                  │    │                 │    │
│  └─────────────────┘    └──────────────────┘    └─────────────────┘    │
│           │                      │                        │              │
│           ▼                      ▼                        ▼              │
│  ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐    │
│  │   Validator     │    │  Vote Parser &   │    │   Persistence   │    │
│  │    Registry     │◀───│   Calculator     │───▶│     Layer       │    │
│  │                 │    │                  │    │                 │    │
│  └─────────────────┘    └──────────────────┘    └─────────────────┘    │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────┐    │
│  │                    Infrastructure Layer                         │    │
│  ├────────────────────────────────────────────────────────────────┤    │
│  │  Connection Pool │ Error Handler │ Config Manager │ Metrics    │    │
│  └────────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Data Flow Architecture

```
1. Discovery Phase:
   RPC Node → Discovery Module → Validator Registry
   
2. Subscription Phase:
   Validator Registry → Subscription Manager → gRPC Streams
   
3. Processing Phase:
   gRPC Updates → Vote Parser → Latency Calculator → Storage Manager
   
4. Persistence Phase:
   Storage Manager → Batch Writer → Database/Files
```

### 1.3 Communication Patterns

- **Async Message Passing**: Components communicate via async channels (Tokio channels in Rust)
- **Backpressure Management**: Bounded channels prevent memory overflow
- **Event-Driven Architecture**: Updates trigger processing pipeline
- **Batch Processing**: Aggregates writes for efficiency

## 2. Technology Stack Recommendation

### 2.1 Language Comparison

| Aspect | Rust | TypeScript | Python |
|--------|------|------------|---------|
| **Performance** | ⭐⭐⭐⭐⭐ Native speed, zero-cost abstractions | ⭐⭐⭐ V8 runtime, GC overhead | ⭐⭐ Interpreted, GIL limitations |
| **Concurrency** | ⭐⭐⭐⭐⭐ Tokio async, true parallelism | ⭐⭐⭐ Event loop, worker threads | ⭐⭐ AsyncIO, limited by GIL |
| **Memory Safety** | ⭐⭐⭐⭐⭐ Compile-time guarantees | ⭐⭐⭐ Runtime checks | ⭐⭐ Runtime checks |
| **Solana Ecosystem** | ⭐⭐⭐⭐⭐ Native SDK, best support | ⭐⭐⭐⭐ Good libraries | ⭐⭐⭐ Basic support |
| **Developer Experience** | ⭐⭐⭐ Steep learning curve | ⭐⭐⭐⭐ Familiar syntax | ⭐⭐⭐⭐⭐ Easy to prototype |
| **Binary Distribution** | ⭐⭐⭐⭐⭐ Single binary | ⭐⭐⭐ Requires Node.js | ⭐⭐ Requires Python runtime |

**Recommendation: Rust** - Given the performance requirements (10k+ writes/minute, <1s latency), Rust is the optimal choice. Its native performance, excellent async support via Tokio, and first-class Solana SDK make it ideal for this high-throughput system.

### 2.2 Core Technology Stack

```toml
# Core Dependencies
[dependencies]
tokio = { version = "1.40", features = ["full"] }           # Async runtime
solana-sdk = "2.0"                                          # Solana integration
solana-client = "2.0"                                       # RPC client
solana-vote-program = "2.0"                                 # Vote state parsing
tonic = "0.12"                                              # gRPC client
prost = "0.13"                                              # Protocol buffers
borsh = "1.5"                                               # Serialization

# Storage
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }  # Async database
arrow = "54.0"                                              # Columnar format
parquet = "54.0"                                            # File storage option

# Infrastructure
tracing = "0.1"                                             # Structured logging
tracing-subscriber = "0.3"                                  # Log formatting
config = "0.14"                                             # Configuration
metrics = "0.23"                                            # Metrics collection
anyhow = "1.0"                                              # Error handling
thiserror = "2.0"                                           # Custom errors

# Development
criterion = "0.5"                                           # Benchmarking
proptest = "1.6"                                            # Property testing
```

### 2.3 Database Choice: SQLite

**Rationale:**
- **Embedded**: No separate process, reduces operational complexity
- **Performance**: Excellent for write-heavy workloads with proper configuration
- **Reliability**: ACID compliant, battle-tested
- **Portability**: Single file, easy backup/migration
- **Async Support**: Works well with sqlx

**Alternative**: Parquet files for analytical workloads
- Better compression
- Columnar format ideal for time-series analysis
- Can run both in parallel (SQLite for real-time, Parquet for archival)

## 3. Detailed Component Design

### 3.1 Discovery Module

```rust
pub struct DiscoveryModule {
    rpc_client: Arc<RpcClient>,
    refresh_interval: Duration,
    validator_threshold: u64,  // Minimum stake to track
}

impl DiscoveryModule {
    async fn discover_validators(&self) -> Result<Vec<ValidatorInfo>> {
        // Fetch vote accounts with retries and exponential backoff
        // Filter by active status and minimum stake
        // Return sorted by stake weight for prioritization
    }
    
    async fn run_periodic_refresh(&self, registry: Arc<ValidatorRegistry>) {
        // Use tokio interval for periodic updates
        // Diff with existing set to detect changes
        // Gracefully handle additions/removals
    }
}
```

**Key Design Decisions:**
- Async/await for non-blocking RPC calls
- Configurable refresh interval (default: 1 epoch)
- Stake threshold to filter insignificant validators
- Graceful handling of validator set changes

### 3.2 Subscription Manager

```rust
pub struct SubscriptionManager {
    grpc_client: Arc<GrpcClient>,
    max_concurrent_streams: usize,  // Default: 100
    stream_pool: Arc<Mutex<StreamPool>>,
    backoff_strategy: ExponentialBackoff,
}

pub struct StreamPool {
    active_streams: HashMap<Pubkey, StreamHandle>,
    pending_queue: VecDeque<Pubkey>,
    semaphore: Arc<Semaphore>,  // Rate limiting
}

impl SubscriptionManager {
    async fn subscribe_to_validator(&self, vote_pubkey: Pubkey) -> Result<StreamHandle> {
        // Acquire semaphore permit for rate limiting
        // Create gRPC stream with timeout
        // Register in active streams map
        // Spawn task to handle updates
    }
    
    async fn handle_stream_failure(&self, vote_pubkey: Pubkey, error: Error) {
        // Log error with context
        // Apply backoff strategy
        // Requeue for subscription
    }
}
```

**Handling 1000+ Concurrent Subscriptions:**
- **Connection Pooling**: Reuse gRPC channels
- **Stream Multiplexing**: Group validators into shared streams where possible
- **Rate Limiting**: Semaphore-based throttling to prevent overwhelming the endpoint
- **Adaptive Batching**: Dynamically adjust batch sizes based on performance metrics
- **Circuit Breaker**: Prevent cascade failures

### 3.3 Parser and Calculator Modules

```rust
pub struct VoteParser {
    previous_states: DashMap<Pubkey, VoteState>,  // Thread-safe cache
}

impl VoteParser {
    async fn parse_update(&self, update: AccountUpdate) -> Result<ParsedVote> {
        // Deserialize with borsh
        // Compare with previous state
        // Extract new votes only
        // Update cache atomically
    }
}

pub struct LatencyCalculator {
    current_slot: Arc<AtomicU64>,  // Updated from slot subscription
}

impl LatencyCalculator {
    fn calculate_latencies(&self, parsed_vote: ParsedVote, landed_slot: Slot) -> Vec<VoteLatency> {
        // For each new vote in batch
        // Calculate: landed_slot - voted_slot
        // Add metadata and timestamp
        // Return vector of latencies
    }
}
```

**Optimization Strategies:**
- Lock-free data structures (DashMap) for concurrent access
- Zero-copy deserialization where possible
- Batch processing to amortize overhead
- SIMD operations for bulk calculations

### 3.4 Storage Layer Design

```rust
pub struct StorageManager {
    write_buffer: Arc<Mutex<WriteBuffer>>,
    flush_interval: Duration,
    batch_size: usize,
}

pub struct WriteBuffer {
    latencies: Vec<VoteLatencyRecord>,
    capacity: usize,
}

impl StorageManager {
    async fn buffer_write(&self, record: VoteLatencyRecord) -> Result<()> {
        // Add to buffer
        // Check if flush needed (size or time based)
        // Trigger async flush if needed
    }
    
    async fn flush_batch(&self) -> Result<()> {
        // Prepare batch insert statement
        // Execute within transaction
        // Clear buffer
        // Update metrics
    }
}

// Schema optimizations
pub struct VoteLatencyRecord {
    timestamp: i64,           // Unix timestamp (8 bytes)
    vote_pubkey_id: u32,      // Foreign key to validator table (4 bytes)
    landed_slot: u64,         // 8 bytes
    voted_slot: u64,          // 8 bytes
    latency: u16,             // Capped at 65k slots (2 bytes)
    confirmation_count: u8,   // 1 byte
}
```

**Storage Optimizations:**
- Batch writes with configurable size (default: 1000 records)
- Prepared statements for efficiency
- Index on (timestamp, vote_pubkey_id) for time-series queries
- Partition by time for easier archival
- Compression for older data

### 3.5 Error Handling and Resilience

```rust
pub enum SvlmError {
    #[error("RPC connection failed: {0}")]
    RpcError(#[from] ClientError),
    
    #[error("gRPC stream error: {0}")]
    GrpcError(#[from] tonic::Status),
    
    #[error("Deserialization failed for {pubkey}: {error}")]
    ParseError { pubkey: String, error: String },
    
    #[error("Storage error: {0}")]
    StorageError(#[from] sqlx::Error),
}

pub struct ErrorHandler {
    max_retries: u32,
    backoff: ExponentialBackoff,
}

impl ErrorHandler {
    async fn handle_with_retry<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn() -> Future<Output = Result<T>>,
    {
        // Implement retry logic with exponential backoff
        // Log each attempt
        // Return error after max retries
    }
}
```

**Resilience Patterns:**
- **Circuit Breaker**: Prevent cascading failures
- **Bulkhead**: Isolate failures to specific validators
- **Timeout**: Prevent hanging operations
- **Graceful Degradation**: Continue with partial data
- **Health Checks**: Monitor component status

## 4. Performance Considerations

### 4.1 Achieving <1s Processing Delay

**Strategy:**
1. **Zero-Copy Processing**: Use references instead of cloning data
2. **Lock-Free Algorithms**: DashMap, crossbeam channels
3. **Batch Processing**: Amortize syscall overhead
4. **Memory Pool**: Pre-allocate buffers to reduce allocation
5. **Async I/O**: Never block the runtime

```rust
// Example: Lock-free metrics collection
pub struct Metrics {
    latencies: AtomicU64,
    updates_processed: AtomicU64,
    errors: AtomicU64,
}

impl Metrics {
    fn record_latency(&self, latency: u64) {
        self.latencies.fetch_add(latency, Ordering::Relaxed);
        self.updates_processed.fetch_add(1, Ordering::Relaxed);
    }
}
```

### 4.2 Handling 10k+ Writes/Minute

**Database Optimizations:**
```sql
-- Optimized schema
PRAGMA journal_mode = WAL;              -- Write-ahead logging
PRAGMA synchronous = NORMAL;            -- Balanced durability
PRAGMA cache_size = -64000;             -- 64MB cache
PRAGMA page_size = 4096;                -- Optimal for SSDs

-- Batch insert with prepared statement
INSERT INTO vote_latencies 
    (timestamp, vote_pubkey_id, landed_slot, voted_slot, latency, confirmation_count)
VALUES 
    (?, ?, ?, ?, ?, ?),
    (?, ?, ?, ?, ?, ?),
    ... -- Up to 1000 rows per batch
```

### 4.3 Memory Management for 1000+ Subscriptions

**Memory Budget (per validator):**
- Vote state cache: ~10KB
- Stream buffer: ~4KB  
- Metadata: ~1KB
- **Total**: ~15KB × 1000 = 15MB (very manageable)

**Strategies:**
- Bounded channels (prevent unbounded growth)
- Regular garbage collection of stale data
- Memory-mapped files for large datasets
- Streaming processing (don't load everything)

### 4.4 Concurrency Strategy

```rust
pub struct RuntimeConfig {
    // CPU-bound work (parsing, calculation)
    compute_threads: usize,  // Default: num_cpus
    
    // I/O-bound work (network, disk)
    io_threads: usize,       // Default: num_cpus * 2
    
    // Dedicated thread for critical path
    subscription_threads: usize,  // Default: 4
}

// Task distribution
tokio::spawn(async move {
    // Network I/O on main runtime
});

tokio::task::spawn_blocking(move || {
    // CPU-intensive parsing on blocking pool
});

rayon::spawn(move || {
    // Parallel batch processing
});
```

## 5. Scalability and Extensibility

### 5.1 Modular Architecture

```rust
// Plugin trait for future analyzers
#[async_trait]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, vote: &ParsedVote) -> Result<AnalysisResult>;
    fn name(&self) -> &str;
}

// Registry for dynamic loading
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn Analyzer>>,
}

// Example extension
pub struct AnomalyDetector;

#[async_trait]
impl Analyzer for AnomalyDetector {
    async fn analyze(&self, vote: &ParsedVote) -> Result<AnalysisResult> {
        // Detect unusual patterns
    }
    
    fn name(&self) -> &str {
        "anomaly_detector"
    }
}
```

### 5.2 Horizontal Scaling Options

**Future Considerations:**
1. **Sharding by Validator**: Distribute validators across instances
2. **Read Replicas**: Separate read/write workloads
3. **Stream Processing**: Integrate with Kafka/Pulsar for distribution
4. **Cloud-Native**: Kubernetes deployment with auto-scaling

### 5.3 Testing Strategy

```rust
// Unit test example
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_latency_calculation(
            voted_slot in 0u64..1000000,
            landed_slot in 0u64..1000000,
        ) {
            let latency = calculate_latency(voted_slot, landed_slot);
            if landed_slot >= voted_slot {
                assert_eq!(latency, landed_slot - voted_slot);
            } else {
                assert_eq!(latency, 0); // Invalid case
            }
        }
    }
}

// Integration test
#[tokio::test]
async fn test_full_pipeline() {
    let test_validator = start_test_validator().await;
    let svlm = SvlmSystem::new(test_config()).await;
    
    // Simulate vote
    test_validator.send_vote(test_vote()).await;
    
    // Verify processing
    tokio::time::sleep(Duration::from_secs(2)).await;
    let stored = svlm.storage.get_latest_latency(TEST_PUBKEY).await;
    assert!(stored.is_some());
}
```

## 6. Risk Analysis

### 6.1 Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| gRPC endpoint rate limits | High | Medium | Implement adaptive rate limiting, connection pooling |
| VoteState schema changes | High | Low | Version detection, pluggable deserializers |
| Memory leak in long-running process | High | Medium | Bounded buffers, regular profiling, metrics monitoring |
| Database corruption | High | Low | WAL mode, regular backups, checksums |
| Network partitions | Medium | Medium | Retry logic, circuit breakers, local buffering |

### 6.2 Performance Bottlenecks

**Identified Bottlenecks:**
1. **Deserialization**: Mitigate with parallel processing
2. **Database Writes**: Batch operations, async I/O
3. **Network Latency**: Local RPC node recommended
4. **Lock Contention**: Lock-free data structures

### 6.3 Operational Considerations

**Monitoring Requirements:**
- System metrics (CPU, memory, disk I/O)
- Application metrics (subscriptions active, latencies processed, errors)
- Business metrics (validator coverage, data freshness)

**Deployment Checklist:**
- [ ] Configure systemd service for auto-restart
- [ ] Set up log rotation
- [ ] Configure database backups
- [ ] Monitor disk space
- [ ] Set up alerting thresholds

## 7. Implementation Roadmap

### Phase 1: Core Infrastructure (Week 1-2)
- Set up Rust project structure
- Implement configuration management
- Create error handling framework
- Set up logging and metrics

### Phase 2: Data Collection (Week 3-4)
- Implement Discovery Module
- Build Subscription Manager
- Create gRPC client integration
- Add retry and resilience logic

### Phase 3: Processing Pipeline (Week 5-6)
- Implement Vote Parser
- Build Latency Calculator
- Create storage abstraction
- Optimize for performance

### Phase 4: Production Readiness (Week 7-8)
- Add comprehensive tests
- Performance benchmarking
- Documentation
- Docker containerization
- Deployment scripts

## 8. Conclusion

This architecture provides a solid foundation for building a high-performance Solana Vote Latency Monitor. The design prioritizes:

1. **Performance**: Native Rust with async I/O and optimized data structures
2. **Reliability**: Comprehensive error handling and resilience patterns
3. **Scalability**: Modular design supporting future growth
4. **Maintainability**: Clear separation of concerns and extensive testing

The system is designed to handle the current requirements (1000+ validators, 10k+ writes/minute) with significant headroom for growth. The modular architecture ensures easy extension for future analysis capabilities while maintaining the core focus on efficient data collection.

## Appendix A: Configuration Schema

```toml
# config/svlm.toml
[rpc]
endpoint = "http://localhost:8899"
timeout_ms = 30000
max_retries = 3

[grpc]
endpoint = "http://localhost:10000"
max_concurrent_streams = 100
stream_buffer_size = 1000

[storage]
type = "sqlite"  # or "parquet"
path = "./data/svlm.db"
batch_size = 1000
flush_interval_ms = 5000

[discovery]
refresh_interval_s = 3600  # 1 hour
min_stake_lamports = 1000000000  # 1 SOL

[runtime]
compute_threads = 0  # 0 = auto-detect
io_threads = 0       # 0 = auto-detect

[logging]
level = "info"
format = "json"
file = "./logs/svlm.log"
```

## Appendix B: Monitoring Metrics

```rust
// Prometheus-style metrics
svlm_validators_total{status="active"} 1052
svlm_subscriptions_active 1052
svlm_updates_processed_total 584739
svlm_latencies_recorded_total 2847392
svlm_errors_total{type="parse"} 12
svlm_storage_write_duration_seconds{quantile="0.99"} 0.043
svlm_grpc_stream_duration_seconds{quantile="0.99"} 0.023
svlm_memory_usage_bytes 287493847
```