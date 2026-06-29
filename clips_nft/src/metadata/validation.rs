//! Metadata validation logic.
//!
//! This module provides comprehensive validation for all metadata fields,
//! ensuring compliance with NFT standards and security best practices.

use soroban_sdk::{Env, String, Vec};

use crate::types::Error;
use crate::metadata::types::Attribute;

/// Supported URL protocols for metadata URIs and media fields.
///
/// Only these protocols are allowed to ensure security and compatibility:
/// - `https://` - Secure HTTP
/// - `ipfs://` - IPFS protocol
/// - `ar://` - Arweave protocol
pub const SUPPORTED_PROTOCOLS: &[&str] = &["https://", "ipfs://", "ar://"];

/// Maximum length for a metadata URI (characters).
const MAX_URI_LENGTH: u32 = 512;

/// Maximum length for description field (characters).
const MAX_DESCRIPTION_LENGTH: u32 = 1000;

/// Maximum number of attributes per token.
const MAX_ATTRIBUTES_COUNT: u32 = 50;

/// Maximum length for attribute trait_type (characters).
const MAX_TRAIT_TYPE_LENGTH: u32 = 64;

/// Maximum length for attribute value (characters).
const MAX_TRAIT_VALUE_LENGTH: u32 = 128;

/// Validates a URL against supported protocols.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `url` - The URL to validate
///
/// # Returns
/// - `Ok(())` if the URL has a supported protocol
/// - `Err(Error::UnsupportedProtocol)` if the protocol is not supported
/// - `Err(Error::MalformedUrl)` if the URL is malformed
///
/// # Example
/// ```rust,ignore
/// validate_url(&env, &String::from_str(&env, "https://example.com/image.png"))?;
/// validate_url(&env, &String::from_str(&env, "ipfs://QmHash"))?;
/// ```
pub fn validate_url(env: &Env, url: &String) -> Result<(), Error> {
    if url.len() == 0 {
        return Err(Error::MalformedUrl);
    }

    // Convert String to a slice we can work with
    let url_str = url.to_string();
    
    // Check if URL starts with any supported protocol
    let has_valid_protocol = SUPPORTED_PROTOCOLS
        .iter()
        .any(|protocol| url_str.starts_with(protocol));

    if !has_valid_protocol {
        return Err(Error::UnsupportedProtocol);
    }

    Ok(())
}

/// Validates a metadata URI.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `uri` - The metadata URI to validate
///
/// # Returns
/// - `Ok(())` if valid
/// - `Err(Error::InvalidURI)` if empty or too long
/// - `Err(Error::UnsupportedProtocol)` if protocol is not supported
pub fn validate_metadata_uri(env: &Env, uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    if uri.len() > MAX_URI_LENGTH {
        return Err(Error::InvalidURI);
    }

    validate_url(env, uri)
}

/// Validates an image URL (optional field).
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `image` - Optional image URL to validate
///
/// # Returns
/// - `Ok(())` if None or valid
/// - `Err(Error)` if invalid
pub fn validate_image_url(env: &Env, image: &Option<String>) -> Result<(), Error> {
    if let Some(url) = image {
        if url.len() > 0 {
            if url.len() > MAX_URI_LENGTH {
                return Err(Error::InvalidURI);
            }
            validate_url(env, url)?;
        }
    }
    Ok(())
}

/// Validates an animation URL (optional field).
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `animation_url` - Optional animation URL to validate
///
/// # Returns
/// - `Ok(())` if None or valid
/// - `Err(Error)` if invalid
pub fn validate_animation_url(env: &Env, animation_url: &Option<String>) -> Result<(), Error> {
    if let Some(url) = animation_url {
        if url.len() > 0 {
            if url.len() > MAX_URI_LENGTH {
                return Err(Error::InvalidURI);
            }
            validate_url(env, url)?;
        }
    }
    Ok(())
}

/// Validates an external URL (optional field).
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `external_url` - Optional external URL to validate
///
/// # Returns
/// - `Ok(())` if None or valid
/// - `Err(Error)` if invalid
pub fn validate_external_url(env: &Env, external_url: &Option<String>) -> Result<(), Error> {
    if let Some(url) = external_url {
        if url.len() > 0 {
            if url.len() > MAX_URI_LENGTH {
                return Err(Error::InvalidURI);
            }
            validate_url(env, url)?;
        }
    }
    Ok(())
}

/// Validates a description field (optional).
///
/// # Arguments
/// * `description` - Optional description to validate
///
/// # Returns
/// - `Ok(())` if None or valid
/// - `Err(Error::InvalidURI)` if too long
pub fn validate_description(description: &Option<String>) -> Result<(), Error> {
    if let Some(desc) = description {
        if desc.len() > MAX_DESCRIPTION_LENGTH {
            return Err(Error::InvalidURI);
        }
    }
    Ok(())
}

/// Validates an array of attributes.
///
/// # Arguments
/// * `attributes` - Vector of attributes to validate
///
/// # Returns
/// - `Ok(())` if valid
/// - `Err(Error::InvalidURI)` if validation fails
///
/// # Validation Rules
/// - Maximum 50 attributes per token
/// - trait_type must not be empty and max 64 characters
/// - value must not be empty and max 128 characters
pub fn validate_attributes(attributes: &Vec<Attribute>) -> Result<(), Error> {
    if attributes.len() > MAX_ATTRIBUTES_COUNT {
        return Err(Error::InvalidURI);
    }

    for attr in attributes.iter() {
        if attr.trait_type.len() == 0 || attr.trait_type.len() > MAX_TRAIT_TYPE_LENGTH {
            return Err(Error::InvalidURI);
        }
        if attr.value.len() == 0 || attr.value.len() > MAX_TRAIT_VALUE_LENGTH {
            return Err(Error::InvalidURI);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_protocols() {
        assert_eq!(SUPPORTED_PROTOCOLS.len(), 3);
        assert!(SUPPORTED_PROTOCOLS.contains(&"https://"));
        assert!(SUPPORTED_PROTOCOLS.contains(&"ipfs://"));
        assert!(SUPPORTED_PROTOCOLS.contains(&"ar://"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_URI_LENGTH, 512);
        assert_eq!(MAX_DESCRIPTION_LENGTH, 1000);
        assert_eq!(MAX_ATTRIBUTES_COUNT, 50);
        assert_eq!(MAX_TRAIT_TYPE_LENGTH, 64);
        assert_eq!(MAX_TRAIT_VALUE_LENGTH, 128);
    }
}
