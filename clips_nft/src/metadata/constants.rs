//! Centralized metadata constants for the ClipsNFT contract.
//!
//! This module provides all metadata-related constants used throughout the contract,
//! ensuring consistency and making it easy to update limits and defaults in one place.

use soroban_sdk::{Env, String};

// =============================================================================
// URI and URL Constants
// =============================================================================

/// Maximum length for a metadata URI in characters.
pub const MAX_URI_LENGTH: u32 = 512;

/// Maximum length for a metadata URI in bytes.
pub const MAX_URI_BYTES: u32 = 512;

// =============================================================================
// Description Constants
// =============================================================================

/// Maximum length for a description field in characters.
pub const MAX_DESCRIPTION_LENGTH: u32 = 1000;

// =============================================================================
// Attribute Constants
// =============================================================================

/// Maximum number of attributes per token.
pub const MAX_ATTRIBUTES_COUNT: u32 = 50;

/// Maximum length for an attribute trait_type in characters.
pub const MAX_TRAIT_TYPE_LENGTH: u32 = 64;

/// Maximum length for an attribute value in characters.
pub const MAX_TRAIT_VALUE_LENGTH: u32 = 128;

/// Maximum length for an attribute display_type in characters.
///
/// Mirrors the OpenSea metadata standard `display_type` field, which can hold
/// values like `"number"`, `"boost_percentage"`, `"boost_number"`, `"date"`,
/// or any other custom rendering hint string.
pub const MAX_DISPLAY_TYPE_LENGTH: u32 = 64;

// =============================================================================
// Title Constants
// =============================================================================

/// Maximum length for a title field in characters.
pub const MAX_TITLE_LENGTH: u32 = 200;

// =============================================================================
// Image Constants
// =============================================================================

/// Maximum length for an image MIME type string in characters.
///
/// Covers all standard IANA media types used for NFT images
/// (e.g., `"image/png"`, `"image/jpeg"`, `"image/gif"`, `"image/webp"`).
pub const MAX_MIME_TYPE_LENGTH: u32 = 64;

/// Default image URL used when no image is provided.
pub const DEFAULT_IMAGE: &str = "https://ipfs.io/ipfs/QmDefaultImage";

/// Default MIME type for images.
pub const DEFAULT_IMAGE_MIME_TYPE: &str = "image/png";

/// Default image width in pixels.
pub const DEFAULT_IMAGE_WIDTH: u32 = 640;

/// Default image height in pixels.
pub const DEFAULT_IMAGE_HEIGHT: u32 = 480;

// =============================================================================
// Version Constants
// =============================================================================

/// Default metadata version for new tokens.
pub const DEFAULT_METADATA_VERSION: u32 = 1;

/// Current metadata schema version.
pub const CURRENT_METADATA_VERSION: u32 = 1;

// =============================================================================
// Size Constants
// =============================================================================

/// Default maximum metadata size in bytes (100 KB).
pub const DEFAULT_MAX_METADATA_SIZE: u32 = 102400;

// =============================================================================
// Supported Protocols
// =============================================================================

/// Supported URL protocols for metadata URIs and media fields.
///
/// Only these protocols are allowed to ensure security and compatibility:
/// - `https://` - Secure HTTP
/// - `ipfs://` - IPFS protocol
/// - `ar://` - Arweave protocol
pub const SUPPORTED_PROTOCOLS: &[&str] = &["https://", "ipfs://", "ar://"];

// =============================================================================
// Helper Functions
// =============================================================================

/// Returns the default image URL as a Soroban String.
pub fn default_image(env: &Env) -> String {
    String::from_str(env, DEFAULT_IMAGE)
}

/// Returns the default image MIME type as a Soroban String.
pub fn default_image_mime_type(env: &Env) -> String {
    String::from_str(env, DEFAULT_IMAGE_MIME_TYPE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_set() {
        assert!(MAX_URI_LENGTH > 0);
        assert!(MAX_DESCRIPTION_LENGTH > 0);
        assert!(MAX_ATTRIBUTES_COUNT > 0);
        assert!(MAX_TRAIT_TYPE_LENGTH > 0);
        assert!(MAX_TRAIT_VALUE_LENGTH > 0);
        assert!(MAX_DISPLAY_TYPE_LENGTH > 0);
        assert!(MAX_MIME_TYPE_LENGTH > 0);
        assert!(MAX_TITLE_LENGTH > 0);
        assert!(DEFAULT_METADATA_VERSION > 0);
        assert!(CURRENT_METADATA_VERSION > 0);
        assert!(DEFAULT_MAX_METADATA_SIZE > 0);
    }

    #[test]
    fn test_supported_protocols() {
        assert_eq!(SUPPORTED_PROTOCOLS.len(), 3);
        assert!(SUPPORTED_PROTOCOLS.contains(&"https://"));
        assert!(SUPPORTED_PROTOCOLS.contains(&"ipfs://"));
        assert!(SUPPORTED_PROTOCOLS.contains(&"ar://"));
    }

    #[test]
    fn test_default_image() {
        let env = Env::default();
        assert_eq!(default_image(&env), String::from_str(&env, DEFAULT_IMAGE));
        assert_eq!(
            default_image_mime_type(&env),
            String::from_str(&env, DEFAULT_IMAGE_MIME_TYPE)
        );
    }

    #[test]
    fn test_uri_constants_consistency() {
        // Ensure URI length constants are consistent
        assert_eq!(MAX_URI_LENGTH, MAX_URI_BYTES);
    }
}
