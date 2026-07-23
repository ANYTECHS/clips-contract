//! Metadata-specific error types.
//!
//! This module defines reusable error types for metadata operations,
//! providing clear and specific error messages for different failure scenarios.

use soroban_sdk::contracterror;

/// Metadata-related errors that can occur during validation, storage, and manipulation.
///
/// These errors cover common failure scenarios when working with NFT metadata:
/// - Invalid metadata structure or content
/// - URI validation failures
/// - Size limit violations
/// - Missing required fields
/// - Version incompatibilities
///
/// # Error Codes
///
/// Error codes start at 100 to avoid conflicts with contract-level errors.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MetadataError {
    /// Metadata structure or content is invalid.
    ///
    /// This is a generic error for metadata validation failures that don't
    /// fall into more specific categories.
    InvalidMetadata = 100,

    /// URI is invalid or malformed.
    ///
    /// Returned when a metadata URI fails validation due to:
    /// - Empty URI
    /// - Unsupported protocol
    /// - Malformed URL structure
    InvalidURI = 101,

    /// Metadata size exceeds the maximum allowed limit.
    ///
    /// Returned when the serialized metadata exceeds the configured
    /// maximum size (default: 100 KB).
    MetadataTooLarge = 102,

    /// Required image field is missing.
    ///
    /// Returned when an image is required but not provided.
    /// This is typically used during metadata validation when
    /// image presence is mandatory.
    MissingImage = 103,

    /// Metadata version is not supported.
    ///
    /// Returned when attempting to process metadata with a version
    /// that the contract does not recognize or support.
    UnsupportedVersion = 104,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_are_unique() {
        let errors = vec![
            MetadataError::InvalidMetadata,
            MetadataError::InvalidURI,
            MetadataError::MetadataTooLarge,
            MetadataError::MissingImage,
            MetadataError::UnsupportedVersion,
        ];

        let mut codes = errors.iter().map(|e| *e as u32).collect::<Vec<u32>>();
        codes.sort();
        codes.dedup();

        assert_eq!(codes.len(), 5, "All error codes should be unique");
    }

    #[test]
    fn test_error_codes_start_at_100() {
        assert_eq!(MetadataError::InvalidMetadata as u32, 100);
        assert_eq!(MetadataError::InvalidURI as u32, 101);
        assert_eq!(MetadataError::MetadataTooLarge as u32, 102);
        assert_eq!(MetadataError::MissingImage as u32, 103);
        assert_eq!(MetadataError::UnsupportedVersion as u32, 104);
    }

    #[test]
    fn test_errors_are_cloneable() {
        let error1 = MetadataError::InvalidMetadata;
        let error2 = error1;
        assert_eq!(error1, error2);
    }

    #[test]
    fn test_errors_are_comparable() {
        assert_eq!(MetadataError::InvalidMetadata, MetadataError::InvalidMetadata);
        assert_ne!(MetadataError::InvalidMetadata, MetadataError::InvalidURI);
        assert!(MetadataError::InvalidURI < MetadataError::MetadataTooLarge);
    }
}