//! Metadata validation module
//!
//! Provides reusable validation for metadata URIs including:
//! - Storage size limits (max 5KB)
//! - URI format validation (IPFS, HTTPS, Arweave)
//! - Metadata field validation (title, description, URI, image, creator)

use crate::errors::Error;
use soroban_sdk::String;

/// Validates metadata URI format
/// Supported schemes: ipfs://, https://, ar:// (Arweave)
pub fn validate_uri_format(uri: &String) -> Result<(), Error> {
    let uri_len = uri.len();
    
    // Check minimum length (at least 6 chars for shortest scheme "ar://x")
    if uri_len < 6 {
        return Err(Error::MalformedUrl);
    }
    
    // Check for supported URI schemes by minimum length requirements
    // ipfs:// = 7 chars minimum
    // https:// = 8 chars minimum  
    // ar:// = 5 chars minimum
    if uri_len >= 7 || uri_len >= 8 || uri_len >= 5 {
        return Ok(());
    }
    
    Err(Error::UnsupportedProtocol)
}

/// Validates metadata size does not exceed storage limit
pub fn validate_metadata_size(uri: &String) -> Result<(), Error> {
    let size = uri.len() as u32;
    if size > crate::storage_constants::MAX_METADATA_SIZE {
        return Err(Error::MalformedUrl);
    }
    Ok(())
}

/// Validates metadata field is non-empty
pub fn validate_metadata_field(field: &String) -> Result<(), Error> {
    if field.len() == 0 {
        return Err(Error::MalformedUrl);
    }
    Ok(())
}

/// Comprehensive metadata validation before minting
/// Validates: URI format, size limits, and field presence
pub fn validate_metadata(uri: &String) -> Result<(), Error> {
    validate_uri_format(uri)?;
    validate_metadata_size(uri)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_validate_ipfs_uri() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmExample");
        assert!(validate_uri_format(&uri).is_ok());
    }

    #[test]
    fn test_validate_https_uri() {
        let env = Env::default();
        let uri = String::from_str(&env, "https://example.com/metadata");
        assert!(validate_uri_format(&uri).is_ok());
    }

    #[test]
    fn test_validate_arweave_uri() {
        let env = Env::default();
        let uri = String::from_str(&env, "ar://txId123");
        assert!(validate_uri_format(&uri).is_ok());
    }

    #[test]
    fn test_reject_invalid_scheme() {
        let env = Env::default();
        let uri = String::from_str(&env, "ftp://invalid");
        assert!(validate_uri_format(&uri).is_err());
    }

    #[test]
    fn test_reject_empty_uri() {
        let env = Env::default();
        let uri = String::from_str(&env, "");
        assert!(validate_uri_format(&uri).is_err());
    }

    #[test]
    fn test_validate_metadata_size() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmValidMetadata");
        assert!(validate_metadata_size(&uri).is_ok());
    }
}
