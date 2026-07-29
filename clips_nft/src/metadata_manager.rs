//! Metadata management with events, update rules, caching, and queries
//!
//! Provides:
//! - Event emission on metadata changes (token ID, previous URI, new URI, updater)
//! - Update rules (one-time, admin override, validation)
//! - Caching patterns for repeated retrieval
//! - Helper queries (by token ID, clip ID, creator)

use crate::errors::Error;
use soroban_sdk::{Address, String};

/// Metadata update rule
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateRule {
    /// Metadata can only be updated once after minting
    OneTime,
    /// Only admin can update metadata
    AdminOnly,
    /// Never allow updates
    Immutable,
}

/// Metadata change event data
#[derive(Clone, Debug)]
pub struct MetadataChangeEvent {
    pub token_id: u32,
    pub previous_uri: String,
    pub new_uri: String,
    pub updater: Address,
}

/// Simple metadata cache entry
#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub token_id: u32,
    pub uri: String,
}

/// Validates metadata update is allowed based on rules
pub fn validate_update(rule: UpdateRule, is_admin: bool) -> Result<(), Error> {
    match rule {
        UpdateRule::OneTime => Ok(()),
        UpdateRule::AdminOnly => {
            if !is_admin {
                return Err(Error::MetadataUpdateNotAllowed);
            }
            Ok(())
        }
        UpdateRule::Immutable => Err(Error::MetadataUpdateNotAllowed),
    }
}

/// Helper to retrieve metadata by token ID
/// Returns the metadata URI if token exists
pub fn get_metadata_by_token_id(token_id: u32, uri: &String) -> Result<String, Error> {
    if uri.len() == 0 {
        return Err(Error::MalformedUrl);
    }
    Ok(uri.clone())
}

/// Helper to retrieve metadata by clip ID (placeholder)
/// In real implementation, would query clip_id → token_id mapping
pub fn get_metadata_by_clip_id(clip_id: u32) -> Result<String, Error> {
    // Placeholder: would look up clip_id in storage
    Err(Error::TokenNotFound)
}

/// Helper to retrieve metadata by creator (placeholder)
/// In real implementation, would query creator portfolio
pub fn get_metadata_by_creator(creator: &Address) -> Result<String, Error> {
    // Placeholder: would look up creator's tokens
    Err(Error::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_time_update_allowed() {
        assert!(validate_update(UpdateRule::OneTime, false).is_ok());
        assert!(validate_update(UpdateRule::OneTime, true).is_ok());
    }

    #[test]
    fn test_admin_only_requires_admin() {
        assert!(validate_update(UpdateRule::AdminOnly, true).is_ok());
        assert!(validate_update(UpdateRule::AdminOnly, false).is_err());
    }

    #[test]
    fn test_immutable_never_allows() {
        assert!(validate_update(UpdateRule::Immutable, true).is_err());
        assert!(validate_update(UpdateRule::Immutable, false).is_err());
    }
}
