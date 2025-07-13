# Changelog

## [Unreleased]

### Added
- Complete InfluxDB v2 integration (Phase 3 of migration plan)
  - High-performance storage backend with worker thread pool
  - Buffered writes with automatic flushing (100ms intervals)
  - LRU cache for deduplication (10k entries)
  - Retry logic with exponential backoff
  - Dual storage implementation for migration period
  - Test example for InfluxDB integration
- Auto-insert functionality for missing validators in SQLite storage
- Comprehensive test coverage for validator auto-insertion

### Changed
- Switched from crossbeam channels to tokio::sync::mpsc for async compatibility
- Updated storage trait to support both SQLite and InfluxDB backends

### Fixed
- Fixed blocking channel issue in InfluxDB worker threads
- Resolved foreign key constraint errors with auto-insert mechanism

### Documentation
- Updated InfluxDB migration plan with Phase 3 completion
- Created InfluxDB setup completion guide
- Added detailed configuration and optimization notes