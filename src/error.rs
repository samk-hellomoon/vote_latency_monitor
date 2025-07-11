//! Error types for SVLM
//!
//! This module defines custom error types used throughout the application
//! for better error handling and reporting.

use solana_sdk::pubkey::ParsePubkeyError;
use thiserror::Error;

/// Result type alias using our custom Error
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for SVLM
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Solana RPC errors
    #[error("RPC error: {0}")]
    Rpc(String),

    /// gRPC connection errors
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    /// Database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Parsing errors
    #[error("Parse error: {0}")]
    Parse(String),

    /// Pubkey parsing errors
    #[error("Invalid pubkey: {0}")]
    InvalidPubkey(#[from] ParsePubkeyError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Network errors
    #[error("Network error: {0}")]
    Network(String),

    /// Validator not found
    #[error("Validator not found: {0}")]
    ValidatorNotFound(String),

    /// Invalid vote transaction
    #[error("Invalid vote transaction: {0}")]
    InvalidVote(String),

    /// Metrics collection error
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// Storage full or quota exceeded
    #[error("Storage error: {0}")]
    Storage(String),

    /// Rate limiting error
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Timeout error
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Generic internal error
    #[error("Internal error: {0}")]
    Internal(String),


    /// Other errors from anyhow
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Error {
    /// Create a configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }

    /// Create an RPC error
    pub fn rpc<S: Into<String>>(msg: S) -> Self {
        Self::Rpc(msg.into())
    }

    /// Create a parse error
    pub fn parse<S: Into<String>>(msg: S) -> Self {
        Self::Parse(msg.into())
    }

    /// Create a network error
    pub fn network<S: Into<String>>(msg: S) -> Self {
        Self::Network(msg.into())
    }

    /// Create a validator not found error
    pub fn validator_not_found<S: Into<String>>(pubkey: S) -> Self {
        Self::ValidatorNotFound(pubkey.into())
    }

    /// Create an invalid vote error
    pub fn invalid_vote<S: Into<String>>(msg: S) -> Self {
        Self::InvalidVote(msg.into())
    }

    /// Create a metrics error
    pub fn metrics<S: Into<String>>(msg: S) -> Self {
        Self::Metrics(msg.into())
    }

    /// Create a storage error
    pub fn storage<S: Into<String>>(msg: S) -> Self {
        Self::Storage(msg.into())
    }

    /// Create a rate limit error
    pub fn rate_limit<S: Into<String>>(msg: S) -> Self {
        Self::RateLimit(msg.into())
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(msg: S) -> Self {
        Self::Timeout(msg.into())
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }

    /// Check if this is a retryable error
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Network(_)
                | Error::Rpc(_)
                | Error::Grpc(_)
                | Error::Timeout(_)
                | Error::RateLimit(_)
        )
    }

    /// Get the error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            Error::Config(_) => "config",
            Error::Rpc(_) => "rpc",
            Error::Grpc(_) => "grpc",
            Error::Database(_) => "database",
            Error::Parse(_) | Error::InvalidPubkey(_) => "parse",
            Error::Serialization(_) => "serialization",
            Error::Network(_) => "network",
            Error::ValidatorNotFound(_) => "validator",
            Error::InvalidVote(_) => "vote",
            Error::Metrics(_) => "metrics",
            Error::Storage(_) => "storage",
            Error::RateLimit(_) => "rate_limit",
            Error::Timeout(_) => "timeout",
            Error::Internal(_) => "internal",
            Error::Other(_) => "other",
        }
    }
    
    /// Get a sanitized error message suitable for external/production use
    /// 
    /// This method returns a generic error message that doesn't expose
    /// sensitive internal details. The original error details should
    /// only be logged internally.
    pub fn external_message(&self) -> String {
        match self {
            Error::Config(_) => "Configuration error occurred".to_string(),
            Error::Rpc(_) => "RPC service error".to_string(),
            Error::Grpc(_) => "gRPC service error".to_string(),
            Error::Database(_) => "Database operation failed".to_string(),
            Error::Parse(_) | Error::InvalidPubkey(_) => "Invalid input format".to_string(),
            Error::Serialization(_) => "Data serialization error".to_string(),
            Error::Network(_) => "Network connection error".to_string(),
            Error::ValidatorNotFound(pubkey) => format!("Validator {} not found", pubkey),
            Error::InvalidVote(_) => "Invalid vote transaction".to_string(),
            Error::Metrics(_) => "Metrics collection error".to_string(),
            Error::Storage(_) => "Storage operation failed".to_string(),
            Error::RateLimit(_) => "Rate limit exceeded".to_string(),
            Error::Timeout(_) => "Operation timed out".to_string(),
            Error::Internal(_) => "Internal service error".to_string(),
            Error::Other(_) => "An error occurred".to_string(),
        }
    }
}

/// Convert from config::ConfigError
impl From<config::ConfigError> for Error {
    fn from(err: config::ConfigError) -> Self {
        Error::Config(err.to_string())
    }
}

/// Convert from serde_json::Error
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

/// Convert from bincode::Error
impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

/// Convert from reqwest::Error
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Error::Timeout(err.to_string())
        } else if err.is_connect() || err.is_request() {
            Error::Network(err.to_string())
        } else {
            Error::Rpc(err.to_string())
        }
    }
}

/// Convert from tokio::time::error::Elapsed
impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Error::Timeout("Operation timed out".to_string())
    }
}

/// Convert from solana_client::client_error::ClientError
impl From<solana_client::client_error::ClientError> for Error {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        Error::Rpc(format!("Solana client error: {}", err))
    }
}

/// Convert from borsh::io::Error
impl From<borsh::io::Error> for Error {
    fn from(err: borsh::io::Error) -> Self {
        Error::Parse(format!("Borsh deserialization error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::config("Invalid configuration");
        assert!(matches!(err, Error::Config(_)));
        assert_eq!(err.to_string(), "Configuration error: Invalid configuration");
    }

    #[test]
    fn test_error_category() {
        assert_eq!(Error::config("test").category(), "config");
        assert_eq!(Error::rpc("test").category(), "rpc");
        assert_eq!(Error::network("test").category(), "network");
        assert_eq!(Error::timeout("test").category(), "timeout");
    }

    #[test]
    fn test_retryable_errors() {
        assert!(Error::network("test").is_retryable());
        assert!(Error::rpc("test").is_retryable());
        assert!(Error::timeout("test").is_retryable());
        assert!(Error::rate_limit("test").is_retryable());
        
        assert!(!Error::config("test").is_retryable());
        assert!(!Error::parse("test").is_retryable());
        assert!(!Error::invalid_vote("test").is_retryable());
    }
    
    #[test]
    fn test_error_constructors() {
        let err = Error::validator_not_found("test-pubkey");
        assert!(matches!(err, Error::ValidatorNotFound(_)));
        assert_eq!(err.to_string(), "Validator not found: test-pubkey");
        
        let err = Error::invalid_vote("bad vote");
        assert!(matches!(err, Error::InvalidVote(_)));
        assert_eq!(err.to_string(), "Invalid vote transaction: bad vote");
        
        let err = Error::metrics("metrics failed");
        assert!(matches!(err, Error::Metrics(_)));
        assert_eq!(err.to_string(), "Metrics error: metrics failed");
        
        let err = Error::storage("disk full");
        assert!(matches!(err, Error::Storage(_)));
        assert_eq!(err.to_string(), "Storage error: disk full");
        
        let err = Error::internal("unexpected error");
        assert!(matches!(err, Error::Internal(_)));
        assert_eq!(err.to_string(), "Internal error: unexpected error");
    }
    
    #[test]
    fn test_error_conversions() {
        // Test config error conversion
        let config_err = config::ConfigError::Message("config error".to_string());
        let err: Error = config_err.into();
        assert!(matches!(err, Error::Config(_)));
        
        // Test serde_json error conversion
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Serialization(_)));
        
    }
    
    #[test]
    fn test_error_display() {
        let err = Error::config("bad config");
        assert_eq!(format!("{}", err), "Configuration error: bad config");
        
        let err = Error::rpc("connection failed");
        assert_eq!(format!("{}", err), "RPC error: connection failed");
        
        let err = Error::network("timeout");
        assert_eq!(format!("{}", err), "Network error: timeout");
    }
    
    #[test]
    fn test_timeout_error_conversion() {
        // Create a timeout error by running a timeout that will expire
        let timeout_result = tokio::time::timeout(std::time::Duration::from_nanos(1), async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        });
        
        // This should timeout and return an Elapsed error
        let rt = tokio::runtime::Runtime::new().unwrap();
        let elapsed = rt.block_on(timeout_result).unwrap_err();
        let err: Error = elapsed.into();
        assert!(matches!(err, Error::Timeout(_)));
        assert!(err.is_retryable());
    }
    
    #[test]
    fn test_result_type_alias() {
        fn test_function() -> Result<String> {
            Ok("success".to_string())
        }
        
        let result = test_function();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        
        fn test_error_function() -> Result<String> {
            Err(Error::internal("test error"))
        }
        
        let result = test_error_function();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_external_message() {
        // Test that external messages don't expose internal details
        let err = Error::config("sensitive config path: /etc/secret");
        assert_eq!(err.external_message(), "Configuration error occurred");
        assert_ne!(err.external_message(), err.to_string());
        
        let err = Error::Database(sqlx::Error::RowNotFound);
        assert_eq!(err.external_message(), "Database operation failed");
        
        let err = Error::rpc("connection to 192.168.1.1 failed");
        assert_eq!(err.external_message(), "RPC service error");
        
        // ValidatorNotFound includes the pubkey (it's public info)
        let err = Error::validator_not_found("ABC123");
        assert_eq!(err.external_message(), "Validator ABC123 not found");
        
        let err = Error::internal("panic at line 42");
        assert_eq!(err.external_message(), "Internal service error");
    }
}