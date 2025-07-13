//! Storage module for different backends

pub mod influxdb_storage;
pub mod dual_storage;

pub use influxdb_storage::InfluxDBStorage;
pub use dual_storage::{DualStorage, MigrationStatus};