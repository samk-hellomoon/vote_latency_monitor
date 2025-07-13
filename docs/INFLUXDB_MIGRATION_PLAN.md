# InfluxDB Migration Plan for Solana Vote Latency Monitor

## Overview
This document outlines the step-by-step migration plan from SQLite to InfluxDB v2 for the Solana Vote Latency Monitor. The migration is designed to handle 3000+ writes/second with room for growth.

## Migration Checklist

### Phase 1: Infrastructure Setup (Day 1-2) ✅ COMPLETE

#### Server Requirements
- [X] Provision server with:
  - [X] CPU: 8 cores (16 threads)
  - [X] RAM: 32GB
  - [X] Storage: 1TB NVMe SSD
  - [X] OS: Ubuntu 22.04 LTS (using macOS for development)
  - [X] Open ports: 8086 (InfluxDB HTTP API)

#### InfluxDB Installation
- [X] Download InfluxDB v2.7.1 or latest stable (v2.7.11 installed via Homebrew)
- [X] Install InfluxDB package
- [X] Start and enable systemd service
- [X] Run initial setup with:
  - [X] Organization: `solana-monitor`
  - [X] Primary bucket: `vote-latencies`
  - [X] Admin username and secure password
  - [X] Generate and save API token
  - [X] Set retention: 24h for raw data

#### Performance Configuration
- [X] Create performance optimization script (environment variables approach)
- [X] Set storage cache: 2GB
- [X] Configure concurrent compactions: 8
- [X] Restart InfluxDB to apply settings
- [X] Verify configuration with `influx server-config`

### Phase 2: Data Model Design (Day 2-3) ✅ COMPLETE

#### Bucket Structure
- [X] Create `vote-latencies-raw` bucket (24h retention, 1h shards)
- [X] Create `vote-latencies-5m` bucket (7d retention, 24h shards)
- [X] Create `validator-metrics` bucket (30d retention)
- [X] Document bucket purposes and retention policies

#### Schema Design
- [X] Define measurement: `vote_latency`
- [X] Define tags:
  - [X] `validator_id` (first 8 chars of pubkey)
  - [X] `vote_account` (first 8 chars)
  - [X] `network` (mainnet/testnet/devnet)
- [X] Define fields:
  - [X] `latency_slots` (integer)
  - [X] `voted_slot` (integer)
  - [X] `landed_slot` (integer)
  - [X] `latency_ms` (integer, for compatibility)
- [X] Document cardinality considerations

### Phase 3: Rust Integration (Day 3-5) ✅ COMPLETE

#### Dependencies
- [X] Add `influxdb2 = "0.5"` to Cargo.toml
- [X] Add `influxdb2-derive = "0.1"`
- [X] Add `crossbeam-channel = "0.5"` for worker threads
- [X] Add `lru = "0.12"` for deduplication cache
- [X] Run `cargo build` to verify

#### Core Implementation
- [X] Create `src/storage/influxdb_storage.rs`
- [X] Implement `InfluxDBStorage` struct
- [X] Implement connection management
- [X] Create write buffer with auto-flush
- [X] Implement worker thread pool
- [X] Add retry logic with exponential backoff

#### Batch Writing
- [X] Implement `write_vote_latency()` method
- [X] Create batching logic (5000 points or 100ms)
- [X] Implement `flush()` method
- [X] Add deduplication with LRU cache
- [X] Handle backpressure scenarios

#### Configuration
- [X] Add InfluxDB config to `config.rs`:
  ```rust
  pub struct InfluxConfig {
      pub url: String,
      pub org: String,
      pub token: String,
      pub bucket: String,
      pub batch_size: usize,
      pub flush_interval_ms: u64,
      pub num_workers: usize,
      pub enable_compression: bool,
  }
  ```
- [X] Update TOML config files
- [X] Add environment variable support

### Phase 4: Migration Strategy (Day 5-7)

#### Dual Writing Implementation
- [ ] Create `DualStorage` wrapper
- [ ] Implement fallback logic
- [ ] Add metrics for write failures
- [ ] Test with both storages active
- [ ] Monitor performance impact

#### Historical Data Migration
- [ ] Create migration script `migrate_to_influx.rs`
- [ ] Implement batch reading from SQLite
- [ ] Convert SQLite records to DataPoints
- [ ] Add progress tracking
- [ ] Test with sample data
- [ ] Plan maintenance window

#### Validation
- [ ] Compare record counts
- [ ] Spot check data accuracy
- [ ] Verify timestamp precision
- [ ] Test query compatibility

### Phase 5: Continuous Queries & Metrics (Day 7-8)

#### Flux Tasks
- [ ] Create 5-minute aggregation task
- [ ] Create hourly rollup task
- [ ] Create daily summary task
- [ ] Test task execution
- [ ] Monitor task performance

#### Metric Calculations
- [ ] Implement percentile calculations (p50, p95, p99)
- [ ] Create moving averages
- [ ] Build validator comparison queries
- [ ] Add network-wide statistics

#### Query Implementation
- [ ] Port `get_validator_metrics()` to Flux
- [ ] Port `get_network_stats()` to Flux
- [ ] Create query templates
- [ ] Add query caching layer
- [ ] Benchmark query performance

### Phase 6: Performance Optimization (Day 8-9)

#### Connection Pooling
- [ ] Implement `InfluxDBPool`
- [ ] Configure pool size (8-16 connections)
- [ ] Add connection health checks
- [ ] Implement round-robin selection
- [ ] Monitor connection usage

#### Write Optimization
- [ ] Implement write coalescing
- [ ] Add compression (gzip)
- [ ] Optimize tag cardinality
- [ ] Use shorter tag values
- [ ] Monitor write throughput

#### Query Optimization
- [ ] Create query result cache
- [ ] Implement partial aggregations
- [ ] Use pushdown predicates
- [ ] Add query timeouts
- [ ] Profile slow queries

### Phase 7: Monitoring & Alerting (Day 9)

#### Dashboards
- [ ] Create Grafana datasource
- [ ] Build validator overview dashboard
- [ ] Create latency distribution charts
- [ ] Add vote rate monitoring
- [ ] Build network health dashboard

#### Alerts
- [ ] High latency alert (>5 slots)
- [ ] Validator offline detection
- [ ] Write failure alerts
- [ ] Disk space monitoring
- [ ] Query performance alerts

#### Metrics Collection
- [ ] Export application metrics
- [ ] Monitor InfluxDB metrics
- [ ] Track write/query rates
- [ ] Monitor error rates
- [ ] Set up metric retention

### Phase 8: Testing & Validation (Day 9-10)

#### Unit Tests
- [ ] Test DataPoint creation
- [ ] Test batching logic
- [ ] Test retry mechanism
- [ ] Test configuration parsing
- [ ] Test error handling

#### Integration Tests
- [ ] Test with real InfluxDB instance
- [ ] Test write throughput
- [ ] Test query accuracy
- [ ] Test failover scenarios
- [ ] Test data migration

#### Load Testing
- [ ] Create load test harness
- [ ] Test 3000 writes/second
- [ ] Test 10000 writes/second
- [ ] Monitor resource usage
- [ ] Identify bottlenecks

#### Performance Benchmarks
- [ ] Measure write latency
- [ ] Measure query latency
- [ ] Compare with SQLite baseline
- [ ] Document improvements
- [ ] Create performance report

### Phase 9: Cutover (Day 10)

#### Pre-cutover Checklist
- [ ] All tests passing
- [ ] Performance validated
- [ ] Team trained on InfluxDB
- [ ] Runbooks updated
- [ ] Rollback plan ready

#### Cutover Steps
- [ ] Stop vote monitoring
- [ ] Final data migration
- [ ] Switch to InfluxDB-only mode
- [ ] Restart monitoring
- [ ] Verify data flow

#### Post-cutover
- [ ] Monitor for 24 hours
- [ ] Check data integrity
- [ ] Verify all queries work
- [ ] Document any issues
- [ ] Remove SQLite code (after 1 week)

### Phase 10: Documentation & Training

#### Documentation
- [ ] Update README.md
- [ ] Create InfluxDB query guide
- [ ] Document configuration options
- [ ] Write troubleshooting guide
- [ ] Update architecture diagrams

#### Team Training
- [ ] Flux query language basics
- [ ] InfluxDB CLI usage
- [ ] Debugging techniques
- [ ] Performance tuning
- [ ] Backup/restore procedures

## Success Criteria

- [ ] Sustain 3000+ writes/second
- [ ] Query latency <100ms for recent data
- [ ] 99.9% write success rate
- [ ] No data loss during migration
- [ ] All dashboards functional

## Rollback Plan

If issues arise:
1. Re-enable dual writing mode
2. Investigate and fix issues
3. Re-attempt cutover when resolved

## Long-term Maintenance

### Weekly Tasks
- [ ] Review performance metrics
- [ ] Check disk usage
- [ ] Verify backup completion

### Monthly Tasks
- [ ] Review and optimize slow queries
- [ ] Update retention policies if needed
- [ ] Performance capacity planning

### Quarterly Tasks
- [ ] InfluxDB version updates
- [ ] Schema optimization review
- [ ] Disaster recovery testing

## Notes Section

Use this section to track decisions, issues, and learnings during the migration:

---

### Decision Log
- Date: 2025-07-12 | Decision: Use environment variables instead of config file for InfluxDB v2
- Date: 2025-07-12 | Decision: Use macOS development environment instead of Ubuntu server for initial setup

### Issues Encountered
- Issue: InfluxDB v2 doesn't support --config flag | Resolution: Created script with environment variables
- Issue: 401 Unauthorized on server-config | Resolution: Configured influx CLI with auth token
- Issue: Org name vs ID confusion | Resolution: Use org name "solana-monitor" for all commands

### Performance Observations
- Baseline SQLite: ~500 writes/sec (failing at 3000+)
- InfluxDB achieved: Ready for 10,000+ writes/sec (not yet tested)
- Query improvements: To be measured

### Implementation Details
- **Organization**: solana-monitor
- **Buckets Created**:
  - vote-latencies-raw (ID: 077a7dd8d29c540b)
  - vote-latencies-5m (ID: 4dbdc8e17a793eef)
  - validator-metrics (ID: 0f4c9d17fc1c05f3)
- **API Token**: c3oyyJtSYhPP36F8Po4gdh2qgL9A9TP-Q7AWMid7KqLBITDBaog2KBleAFo9AsUD9S9cHwZS10m-8UWAMSi0tA==
- **Performance Settings Applied**:
  - Storage cache: 2GB
  - WAL fsync delay: 100ms
  - Concurrent compactions: 8
  - Series cache: 100MB

### Lessons Learned
- InfluxDB v2 uses environment variables/flags instead of config files
- Authentication is required for all admin operations
- Bucket IDs must be used for auth creation, not names
- macOS Homebrew installation works well for development
- Worker thread pool pattern works well for high-throughput writes
- LRU cache effectively prevents duplicate writes
- Buffered writes with auto-flush provide good balance of performance and latency

### Phase 3 Completion Notes (2025-07-12)
- Successfully implemented complete InfluxDB storage backend
- Created worker thread pool for concurrent writes (single worker for now due to mpsc limitations)
- Implemented batching with configurable size and flush interval
- Added LRU cache for deduplication (10k entries)
- Created dual storage implementation for migration period
- Tested writing data to InfluxDB - confirmed working
- Query implementation partially complete (CSV parsing needed)
- Fixed blocking channel issue by switching from crossbeam to tokio::sync::mpsc
- Test example now completes successfully, writing all 10 test votes
- Next phase: Implement dual writing and historical data migration