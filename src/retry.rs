//! Retry logic infrastructure for SVLM
//!
//! This module provides a reusable retry mechanism with exponential backoff
//! for handling transient failures in network operations.

use crate::error::{Error, Result};
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    
    /// Initial delay before first retry
    pub initial_delay: Duration,
    
    /// Maximum delay between retries
    pub max_delay: Duration,
    
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    
    /// Add random jitter to delays
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the maximum number of attempts
    pub fn with_max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }
    
    /// Set the initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }
    
    /// Set the maximum delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }
    
    /// Set the backoff multiplier
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }
    
    /// Enable or disable jitter
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }
}

/// Retry policy trait for custom retry logic
pub trait RetryPolicy: Send + Sync {
    /// Determine if an error should trigger a retry
    fn should_retry(&self, error: &Error) -> bool;
    
    /// Calculate the delay before the next retry attempt
    fn next_delay(&self, attempt: u32, base_delay: Duration) -> Duration;
}

/// Default retry policy implementation
pub struct DefaultRetryPolicy {
    config: RetryConfig,
}

impl DefaultRetryPolicy {
    /// Create a new default retry policy
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }
}

impl RetryPolicy for DefaultRetryPolicy {
    fn should_retry(&self, error: &Error) -> bool {
        error.is_retryable()
    }
    
    fn next_delay(&self, attempt: u32, base_delay: Duration) -> Duration {
        let mut delay = base_delay.mul_f64(self.config.backoff_multiplier.powi(attempt as i32));
        
        // Cap at max delay
        if delay > self.config.max_delay {
            delay = self.config.max_delay;
        }
        
        // Add jitter if enabled
        if self.config.jitter {
            use rand::Rng;
            let jitter_range = delay.as_millis() as f64 * 0.1; // 10% jitter
            let jitter = rand::thread_rng().gen_range(-jitter_range..=jitter_range);
            let jittered_millis = (delay.as_millis() as f64 + jitter).max(0.0) as u64;
            delay = Duration::from_millis(jittered_millis);
        }
        
        delay
    }
}

/// Execute an operation with retry logic
pub async fn retry_with_policy<F, Fut, T, P>(
    operation: F,
    policy: P,
    config: &RetryConfig,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
    P: RetryPolicy,
{
    let mut attempt = 0;
    let mut _last_error = None;
    
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                attempt += 1;
                
                if attempt >= config.max_attempts || !policy.should_retry(&error) {
                    warn!(
                        attempt,
                        max_attempts = config.max_attempts,
                        error = %error,
                        "Operation failed after retries"
                    );
                    return Err(error);
                }
                
                let delay = policy.next_delay(attempt - 1, config.initial_delay);
                debug!(
                    attempt,
                    delay_ms = delay.as_millis(),
                    error = %error,
                    "Retrying operation after delay"
                );
                
                _last_error = Some(error);
                sleep(delay).await;
            }
        }
    }
}

/// Execute an operation with default retry logic
pub async fn retry<F, Fut, T>(operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let config = RetryConfig::default();
    let policy = DefaultRetryPolicy::new(config.clone());
    retry_with_policy(operation, policy, &config).await
}

/// Execute an operation with custom retry configuration
pub async fn retry_with_config<F, Fut, T>(
    operation: F,
    config: RetryConfig,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let policy = DefaultRetryPolicy::new(config.clone());
    retry_with_policy(operation, policy, &config).await
}

/// Builder for creating retry operations
pub struct RetryBuilder {
    config: RetryConfig,
}

impl RetryBuilder {
    /// Create a new retry builder
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }
    
    /// Set the maximum number of attempts
    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.config.max_attempts = attempts;
        self
    }
    
    /// Set the initial delay
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.config.initial_delay = delay;
        self
    }
    
    /// Set the maximum delay
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.config.max_delay = delay;
        self
    }
    
    /// Set the backoff multiplier
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.config.backoff_multiplier = multiplier;
        self
    }
    
    /// Enable or disable jitter
    pub fn jitter(mut self, jitter: bool) -> Self {
        self.config.jitter = jitter;
        self
    }
    
    /// Execute the operation with the configured retry logic
    pub async fn run<F, Fut, T>(self, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        retry_with_config(operation, self.config).await
    }
}

impl Default for RetryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let result = retry(|| async {
            let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                Err(Error::network("Temporary failure"))
            } else {
                Ok("Success")
            }
        })
        .await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
    
    #[tokio::test]
    async fn test_retry_exhausted_attempts() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let config = RetryConfig::default()
            .with_max_attempts(2)
            .with_initial_delay(Duration::from_millis(10));
        
        let result = retry_with_config(|| async {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            Err::<String, _>(Error::network("Persistent failure"))
        }, config)
        .await;
        
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
    
    #[tokio::test]
    async fn test_non_retryable_error() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let result = retry(|| async {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            Err::<String, _>(Error::config("Configuration error"))
        })
        .await;
        
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1); // Should not retry
    }
    
    #[tokio::test]
    async fn test_retry_builder() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let result = RetryBuilder::new()
            .max_attempts(5)
            .initial_delay(Duration::from_millis(5))
            .backoff_multiplier(1.5)
            .jitter(false)
            .run(|| async {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
                if count < 3 {
                    Err(Error::network("Temporary failure"))
                } else {
                    Ok("Success")
                }
            })
            .await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");
        assert_eq!(attempts.load(Ordering::SeqCst), 4);
    }
    
    #[test]
    fn test_exponential_backoff_calculation() {
        let config = RetryConfig::default().with_jitter(false);
        let policy = DefaultRetryPolicy::new(config);
        
        let base = Duration::from_millis(100);
        
        let delay0 = policy.next_delay(0, base);
        let delay1 = policy.next_delay(1, base);
        let delay2 = policy.next_delay(2, base);
        
        assert_eq!(delay0, Duration::from_millis(100));
        assert_eq!(delay1, Duration::from_millis(200));
        assert_eq!(delay2, Duration::from_millis(400));
    }
}