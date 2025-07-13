# Changelog

## [Unreleased]

### Added
- Security validation exception for InfluxDB localhost connections
- Progress notes documentation
- Hellomoon endpoint configuration support

### Removed
- SQLite storage backend (replaced by InfluxDB)
- Dual storage complexity
- SQLite dependencies and configuration
- Database initialization command
- Migration script (no longer needed)
- Obsolete examples that referenced SQLite

### Added
- Complete InfluxDB v2 integration (Phase 3 of migration plan)
  - High-performance storage backend with worker thread pool
  - Buffered writes with automatic flushing (100ms intervals)
  - LRU cache for deduplication (10k entries)
  - Retry logic with exponential backoff
  - Dual storage implementation for migration period
  - Test example for InfluxDB integration
- Dual storage support (Phase 4 of migration plan)
  - Automatic dual writing when InfluxDB is configured
  - Fallback to SQLite on InfluxDB errors
  - Migration script for historical data
  - Batch fetching methods for efficient migration
  - Test example for dual storage verification
  - Comprehensive metrics tracking for dual storage operations
- Flux queries and aggregations (Phase 5 of migration plan)
  - 5-minute aggregation task for vote latency metrics
  - Hourly rollup task for long-term analysis
  - Daily summary task with percentile calculations
  - Query templates for common operations
  - Script to deploy Flux tasks to InfluxDB
- Auto-insert functionality for missing validators in SQLite storage
- Comprehensive test coverage for validator auto-insertion
- Test data generator for migration testing

### Changed
- Switched from crossbeam channels to tokio::sync::mpsc for async compatibility
- Storage now uses InfluxDB exclusively for better performance at scale
- Simplified storage configuration - InfluxDB is now required
- Updated documentation to reflect InfluxDB-only architecture
- Reduced default max_subscriptions from 50 to 10 for stability

### Fixed
- Fixed blocking channel issue in InfluxDB worker threads
- Resolved foreign key constraint errors with auto-insert mechanism
- Fixed security validation blocking localhost InfluxDB connections
- Fixed configuration validation tests for new InfluxDB requirements

### Documentation
- Updated InfluxDB migration plan with Phase 3 completion and Phase 4 progress
- Created InfluxDB setup completion guide
- Added detailed configuration and optimization notes
- Created sample configuration with InfluxDB settings