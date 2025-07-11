//! Security utilities module
//!
//! This module provides common security utilities for input validation,
//! sanitization, and secure handling of user inputs.

use anyhow::{anyhow, Result};
use solana_sdk::pubkey::Pubkey;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use url::Url;

/// Maximum allowed length for string fields
pub const MAX_STRING_LENGTH: usize = 1024;

/// Maximum allowed length for descriptions
pub const MAX_DESCRIPTION_LENGTH: usize = 4096;

/// Maximum allowed URL length
pub const MAX_URL_LENGTH: usize = 2048;

/// Validates a Solana pubkey string
///
/// # Arguments
/// * `pubkey_str` - The pubkey string to validate
///
/// # Returns
/// * `Ok(Pubkey)` if valid
/// * `Err(String)` with error message if invalid
pub fn validate_pubkey(pubkey_str: &str) -> Result<Pubkey> {
    // Check length first (base58 encoded pubkey should be 32-44 chars)
    if pubkey_str.is_empty() || pubkey_str.len() > 44 {
        return Err(anyhow!("Invalid pubkey length"));
    }
    
    // Check for valid base58 characters
    if !pubkey_str.chars().all(|c| {
        c.is_ascii_alphanumeric() && c != '0' && c != 'O' && c != 'I' && c != 'l'
    }) {
        return Err(anyhow!("Invalid characters in pubkey"));
    }
    
    // Try to parse
    Pubkey::from_str(pubkey_str)
        .map_err(|e| anyhow!("Invalid pubkey format: {}", e))
}

/// Validates a URL string
///
/// # Arguments
/// * `url_str` - The URL string to validate
/// * `allowed_schemes` - Optional list of allowed URL schemes (e.g., ["http", "https"])
///
/// # Returns
/// * `Ok(String)` with normalized URL if valid
/// * `Err(String)` with error message if invalid
pub fn validate_url(url_str: &str, allowed_schemes: Option<&[&str]>) -> Result<String> {
    // Check length
    if url_str.is_empty() {
        return Err(anyhow!("URL cannot be empty"));
    }
    
    if url_str.len() > MAX_URL_LENGTH {
        return Err(anyhow!("URL exceeds maximum length of {} characters", MAX_URL_LENGTH));
    }
    
    // Parse URL
    let url = Url::parse(url_str)
        .map_err(|e| anyhow!("Invalid URL format: {}", e))?;
    
    // Check scheme
    if let Some(schemes) = allowed_schemes {
        if !schemes.contains(&url.scheme()) {
            return Err(anyhow!("URL scheme '{}' not allowed. Allowed schemes: {:?}", 
                url.scheme(), schemes));
        }
    } else {
        // Default to only allowing http and https
        if !["http", "https"].contains(&url.scheme()) {
            return Err(anyhow!("Only HTTP and HTTPS URLs are allowed"));
        }
    }
    
    // Check for localhost/private IPs in production
    if let Some(host) = url.host_str() {
        if is_private_host(host) {
            return Err(anyhow!("URLs to private/local addresses are not allowed"));
        }
    }
    
    Ok(url.to_string())
}

/// Validates a file path to prevent path traversal attacks
///
/// # Arguments
/// * `path_str` - The path string to validate
/// * `base_dir` - Optional base directory to restrict paths to
///
/// # Returns
/// * `Ok(PathBuf)` with canonicalized path if valid
/// * `Err(String)` with error message if invalid
pub fn validate_path(path_str: &str, base_dir: Option<&Path>) -> Result<PathBuf> {
    if path_str.is_empty() {
        return Err(anyhow!("Path cannot be empty"));
    }
    
    // Check for suspicious patterns
    if path_str.contains("..") || path_str.contains("~") {
        return Err(anyhow!("Path contains suspicious patterns"));
    }
    
    let path = Path::new(path_str);
    
    // If base_dir is provided, ensure the path is within it
    if let Some(base) = base_dir {
        let absolute_base = base.canonicalize()
            .map_err(|e| anyhow!("Invalid base directory: {}", e))?;
        
        // Resolve the path relative to base if it's relative
        let full_path = if path.is_relative() {
            absolute_base.join(path)
        } else {
            path.to_path_buf()
        };
        
        // Canonicalize to resolve any symlinks
        let canonical = full_path.canonicalize()
            .or_else(|_| {
                // If file doesn't exist yet, canonicalize the parent
                full_path.parent()
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid path"))?
                    .canonicalize()
                    .map(|parent| parent.join(full_path.file_name().unwrap()))
            })
            .map_err(|e| anyhow!("Cannot resolve path: {}", e))?;
        
        // Ensure the resolved path is within the base directory
        if !canonical.starts_with(&absolute_base) {
            return Err(anyhow!("Path escapes base directory"));
        }
        
        Ok(canonical)
    } else {
        // No base directory restriction, just validate the path
        Ok(path.to_path_buf())
    }
}

/// Validates and sanitizes a string field
///
/// # Arguments
/// * `value` - The string to validate
/// * `field_name` - Name of the field for error messages
/// * `max_length` - Maximum allowed length
///
/// # Returns
/// * `Ok(String)` with sanitized string if valid
/// * `Err(String)` with error message if invalid
pub fn validate_string(value: &str, field_name: &str, max_length: usize) -> Result<String> {
    if value.len() > max_length {
        return Err(anyhow!("{} exceeds maximum length of {} characters", 
            field_name, max_length));
    }
    
    // Remove any control characters
    let sanitized: String = value
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect();
    
    // Check if string was modified (contained control chars)
    if sanitized.len() != value.len() {
        return Err(anyhow!("{} contains invalid control characters", field_name));
    }
    
    Ok(sanitized.trim().to_string())
}

/// Checks if a hostname refers to a private/local address
fn is_private_host(host: &str) -> bool {
    // Check for localhost variants
    if host == "localhost" || host == "127.0.0.1" || host == "::1" {
        return true;
    }
    
    // Check for private IP ranges
    if let Ok(addr) = host.parse::<std::net::IpAddr>() {
        match addr {
            std::net::IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                // Private ranges: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                octets[0] == 10
                    || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                    || (octets[0] == 192 && octets[1] == 168)
                    || octets[0] == 127 // loopback
            }
            std::net::IpAddr::V6(ipv6) => {
                ipv6.is_loopback() || ipv6.segments()[0] == 0xfc00 // unique local
            }
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_pubkey() {
        // Valid pubkey
        let valid = "11111111111111111111111111111111";
        assert!(validate_pubkey(valid).is_ok());
        
        // Invalid pubkeys
        assert!(validate_pubkey("").is_err());
        assert!(validate_pubkey("invalid!pubkey").is_err());
        assert!(validate_pubkey("toolongpubkeytoolongpubkeytoolongpubkeytoolongpubkey").is_err());
        assert!(validate_pubkey("0OIl").is_err()); // Contains disallowed base58 chars
    }
    
    #[test]
    fn test_validate_url() {
        // Valid URLs
        assert!(validate_url("https://example.com", None).is_ok());
        assert!(validate_url("http://example.com:8080/path", None).is_ok());
        
        // Custom schemes
        assert!(validate_url("grpc://example.com", Some(&["grpc"])).is_ok());
        
        // Invalid URLs
        assert!(validate_url("", None).is_err());
        assert!(validate_url("not-a-url", None).is_err());
        assert!(validate_url("ftp://example.com", None).is_err());
        assert!(validate_url("http://localhost", None).is_err());
        assert!(validate_url("http://127.0.0.1", None).is_err());
        assert!(validate_url("http://192.168.1.1", None).is_err());
        
        // URL too long
        let long_url = format!("https://example.com/{}", "a".repeat(MAX_URL_LENGTH));
        assert!(validate_url(&long_url, None).is_err());
    }
    
    #[test]
    fn test_validate_path() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();
        
        // Valid paths
        assert!(validate_path("test.db", Some(base_path)).is_ok());
        assert!(validate_path("data/test.db", Some(base_path)).is_ok());
        
        // Invalid paths
        assert!(validate_path("", Some(base_path)).is_err());
        assert!(validate_path("../escape", Some(base_path)).is_err());
        assert!(validate_path("/etc/passwd", Some(base_path)).is_err());
        assert!(validate_path("~/.ssh/id_rsa", Some(base_path)).is_err());
    }
    
    #[test]
    fn test_validate_string() {
        // Valid strings
        assert_eq!(
            validate_string("  hello world  ", "test", 100).unwrap(),
            "hello world"
        );
        
        // String too long
        let long_string = "a".repeat(101);
        assert!(validate_string(&long_string, "test", 100).is_err());
        
        // Control characters
        assert!(validate_string("hello\x00world", "test", 100).is_err());
        assert!(validate_string("hello\x1bworld", "test", 100).is_err());
        
        // Whitespace is allowed
        assert!(validate_string("hello\nworld", "test", 100).is_ok());
    }
    
    #[test]
    fn test_is_private_host() {
        assert!(is_private_host("localhost"));
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("::1"));
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("172.16.0.1"));
        assert!(is_private_host("192.168.1.1"));
        
        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("1.1.1.1"));
    }
}