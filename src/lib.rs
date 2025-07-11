//! Solana Vote Latency Monitor Library
//!
//! This library provides the core functionality for monitoring vote latency
//! across Solana validators. It includes modules for validator discovery,
//! gRPC subscription management, vote parsing, latency calculation, and storage.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod config;
pub mod error;
pub mod metrics;
pub mod models;
pub mod modules;
pub mod retry;
pub mod security;

pub use config::Config;
pub use error::{Error, Result};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::config::Config;
    pub use crate::error::{Error, Result};
    pub use crate::models::{ValidatorInfo, VoteLatency, VoteTransaction};
}

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "svlm");
    }
}