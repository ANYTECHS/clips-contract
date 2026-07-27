//! Metadata validation logic.
//!
//! This module provides comprehensive validation for all metadata fields,
//! ensuring compliance with NFT standards and security best practices.

use soroban_sdk::{Env, String, Vec};

use alloc::format;
use alloc::string::ToString;
use crate::types::Error;
use crate::metadata::types::{Attribute, MetadataImage};
use crate::metadata::constants::*;

/// Supported URL protocols for metadata URIs and media fields.
///
/// Only these protocols are allowed to ensure security and compatibility:
/// - `https://` - Secure HTTP
/// - `ipfs://` - IPFS protocol
/// - `ar://` - Arweave protocol
pub const SUPPORTED_PROTOCOLS: &[&str] = &["https://", "ipfs://", "ar://"];

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

    let url_str = format!("{}", url);

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

/// Validates a `MetadataImage` struct.
///
/// # Arguments
/// * `env`   – The Soroban environment
/// * `image` – The `MetadataImage` to validate
///
/// # Returns
/// - `Ok(())` if all fields are valid
/// - `Err(Error::InvalidURI)` if `image_url` is empty, too long, or has an
///   unsupported protocol, or if `mime_type` is empty / too long
///
/// # Validation Rules
/// - `image_url` must be non-empty, ≤ 512 characters, and use `https://`,
///   `ipfs://`, or `ar://`
/// - `mime_type` must be non-empty and ≤ `MAX_MIME_TYPE_LENGTH` (64) characters
/// - `width` and `height` are unconstrained (`0` is allowed for placeholders)
pub fn validate_metadata_image(env: &Env, image: &MetadataImage) -> Result<(), Error> {
    // Validate image_url
    if image.image_url.len() == 0 {
        return Err(Error::InvalidURI);
    }
    if image.image_url.len() > MAX_URI_LENGTH {
        return Err(Error::InvalidURI);
    }
    validate_url(env, &image.image_url)?;

    // Validate mime_type
    if image.mime_type.len() == 0 {
        return Err(Error::InvalidURI);
    }
    if image.mime_type.len() > MAX_MIME_TYPE_LENGTH {
        return Err(Error::InvalidURI);
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
/// - display_type, when present, must not be empty and max 64 characters
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
        if let Some(ref dt) = attr.display_type {
            if dt.len() == 0 || dt.len() > MAX_DISPLAY_TYPE_LENGTH {
                return Err(Error::InvalidURI);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String, Vec};

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
        assert_eq!(MAX_DISPLAY_TYPE_LENGTH, 64);
    }

    // ========== validate_url tests ==========

    #[test]
    fn test_validate_url_with_https() {
        let env = Env::default();
        let url = String::from_str(&env, "https://example.com/image.png");
        assert!(validate_url(&env, &url).is_ok());
    }

    #[test]
    fn test_validate_url_with_ipfs() {
        let env = Env::default();
        let url = String::from_str(&env, "ipfs://QmHash123");
        assert!(validate_url(&env, &url).is_ok());
    }

    #[test]
    fn test_validate_url_with_arweave() {
        let env = Env::default();
        let url = String::from_str(&env, "ar://abc123xyz");
        assert!(validate_url(&env, &url).is_ok());
    }

    #[test]
    fn test_validate_url_empty_string_fails() {
        let env = Env::default();
        let url = String::from_str(&env, "");
        assert_eq!(validate_url(&env, &url), Err(Error::MalformedUrl));
    }

    #[test]
    fn test_validate_url_unsupported_protocol_fails() {
        let env = Env::default();
        let url = String::from_str(&env, "ftp://example.com/file");
        assert_eq!(validate_url(&env, &url), Err(Error::UnsupportedProtocol));
    }

    #[test]
    fn test_validate_url_http_fails() {
        let env = Env::default();
        let url = String::from_str(&env, "http://example.com");
        assert_eq!(validate_url(&env, &url), Err(Error::UnsupportedProtocol));
    }

    #[test]
    fn test_validate_url_no_protocol_fails() {
        let env = Env::default();
        let url = String::from_str(&env, "example.com");
        assert_eq!(validate_url(&env, &url), Err(Error::UnsupportedProtocol));
    }

    // ========== validate_metadata_uri tests ==========

    #[test]
    fn test_validate_metadata_uri_valid() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmValidHash");
        assert!(validate_metadata_uri(&env, &uri).is_ok());
    }

    #[test]
    fn test_validate_metadata_uri_empty_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "");
        assert_eq!(validate_metadata_uri(&env, &uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_uri_too_long_fails() {
        let env = Env::default();
        let long_uri = String::from_str(&env, &"a".repeat(513));
        assert_eq!(validate_metadata_uri(&env, &long_uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_uri_max_length_ok() {
        let env = Env::default();
        let max_uri = String::from_str(&env, &"a".repeat(512));
        assert!(validate_metadata_uri(&env, &max_uri).is_ok());
    }

    #[test]
    fn test_validate_metadata_uri_unsupported_protocol_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ftp://invalid.com");
        assert_eq!(validate_metadata_uri(&env, &uri), Err(Error::InvalidURI));
    }

    // ========== validate_image_url tests ==========

    #[test]
    fn test_validate_image_url_none_ok() {
        let env = Env::default();
        assert!(validate_image_url(&env, &None).is_ok());
    }

    #[test]
    fn test_validate_image_url_some_valid() {
        let env = Env::default();
        let image = Some(String::from_str(&env, "https://example.com/image.png"));
        assert!(validate_image_url(&env, &image).is_ok());
    }

    #[test]
    fn test_validate_image_url_some_invalid_protocol() {
        let env = Env::default();
        let image = Some(String::from_str(&env, "ftp://example.com/image.png"));
        assert_eq!(validate_image_url(&env, &image), Err(Error::UnsupportedProtocol));
    }

    #[test]
    fn test_validate_image_url_too_long_fails() {
        let env = Env::default();
        let long_url = String::from_str(&env, &format!("https://example.com/{}", "a".repeat(500)));
        let image = Some(long_url);
        assert_eq!(validate_image_url(&env, &image), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_image_url_empty_string_ok() {
        let env = Env::default();
        let image = Some(String::from_str(&env, ""));
        assert!(validate_image_url(&env, &image).is_ok());
    }

    // ========== validate_animation_url tests ==========

    #[test]
    fn test_validate_animation_url_none_ok() {
        let env = Env::default();
        assert!(validate_animation_url(&env, &None).is_ok());
    }

    #[test]
    fn test_validate_animation_url_some_valid() {
        let env = Env::default();
        let anim = Some(String::from_str(&env, "ipfs://QmVideo"));
        assert!(validate_animation_url(&env, &anim).is_ok());
    }

    #[test]
    fn test_validate_animation_url_invalid_protocol_fails() {
        let env = Env::default();
        let anim = Some(String::from_str(&env, "http://insecure.com/video.mp4"));
        assert_eq!(validate_animation_url(&env, &anim), Err(Error::UnsupportedProtocol));
    }

    #[test]
    fn test_validate_animation_url_too_long_fails() {
        let env = Env::default();
        let long_url = String::from_str(&env, &format!("https://example.com/{}", "a".repeat(500)));
        let anim = Some(long_url);
        assert_eq!(validate_animation_url(&env, &anim), Err(Error::InvalidURI));
    }

    // ========== validate_external_url tests ==========

    #[test]
    fn test_validate_external_url_none_ok() {
        let env = Env::default();
        assert!(validate_external_url(&env, &None).is_ok());
    }

    #[test]
    fn test_validate_external_url_some_valid() {
        let env = Env::default();
        let ext = Some(String::from_str(&env, "https://clipcash.com/clip/123"));
        assert!(validate_external_url(&env, &ext).is_ok());
    }

    #[test]
    fn test_validate_external_url_invalid_protocol_fails() {
        let env = Env::default();
        let ext = Some(String::from_str(&env, "file:///path/to/file"));
        assert_eq!(validate_external_url(&env, &ext), Err(Error::UnsupportedProtocol));
    }

    #[test]
    fn test_validate_external_url_too_long_fails() {
        let env = Env::default();
        let long_url = String::from_str(&env, &format!("https://example.com/{}", "a".repeat(500)));
        let ext = Some(long_url);
        assert_eq!(validate_external_url(&env, &ext), Err(Error::InvalidURI));
    }

    // ========== validate_description tests ==========

    #[test]
    fn test_validate_description_none_ok() {
        assert!(validate_description(&None).is_ok());
    }

    #[test]
    fn test_validate_description_some_valid() {
        let desc = Some(String::from_str(&Env::default(), "A great clip"));
        assert!(validate_description(&desc).is_ok());
    }

    #[test]
    fn test_validate_description_too_long_fails() {
        let long_desc = Some(String::from_str(&Env::default(), &"a".repeat(1001)));
        assert_eq!(validate_description(&long_desc), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_description_max_length_ok() {
        let max_desc = Some(String::from_str(&Env::default(), &"a".repeat(1000)));
        assert!(validate_description(&max_desc).is_ok());
    }

    #[test]
    fn test_validate_description_empty_ok() {
        let desc = Some(String::from_str(&Env::default(), ""));
        assert!(validate_description(&desc).is_ok());
    }

    // ========== validate_attributes tests ==========

    #[test]
    fn test_validate_attributes_empty_ok() {
        let env = Env::default();
        let attrs = Vec::new(&env);
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_valid() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "rarity"),
            value: String::from_str(&env, "legendary"),
            display_type: None,
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_too_many_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        for i in 0..51 {
            attrs.push_back(Attribute {
                trait_type: String::from_str(&env, &format!("trait{}", i)),
                value: String::from_str(&env, "value"),
                display_type: None,
            });
        }
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_attributes_max_count_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        for i in 0..50 {
            attrs.push_back(Attribute {
                trait_type: String::from_str(&env, &format!("trait{}", i)),
                value: String::from_str(&env, "value"),
                display_type: None,
            });
        }
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_empty_trait_type_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, ""),
            value: String::from_str(&env, "value"),
            display_type: None,
        });
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_attributes_empty_value_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, ""),
            display_type: None,
        });
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_attributes_trait_type_too_long_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, &"a".repeat(65)),
            value: String::from_str(&env, "value"),
            display_type: None,
        });
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_attributes_value_too_long_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, &"a".repeat(129)),
            display_type: None,
        });
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_attributes_trait_type_max_length_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, &"a".repeat(64)),
            value: String::from_str(&env, "value"),
            display_type: None,
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_value_max_length_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, &"a".repeat(128)),
            display_type: None,
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    // ========== display_type validation tests ==========

    #[test]
    fn test_validate_attributes_display_type_none_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "rarity"),
            value: String::from_str(&env, "legendary"),
            display_type: None,
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_display_type_valid_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "virality_score"),
            value: String::from_str(&env, "98"),
            display_type: Some(String::from_str(&env, "number")),
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_display_type_boost_percentage_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "speed_boost"),
            value: String::from_str(&env, "15"),
            display_type: Some(String::from_str(&env, "boost_percentage")),
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_display_type_date_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "birthday"),
            value: String::from_str(&env, "1546360800"),
            display_type: Some(String::from_str(&env, "date")),
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_display_type_max_length_ok() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, "value"),
            display_type: Some(String::from_str(&env, &"d".repeat(64))),
        });
        assert!(validate_attributes(&attrs).is_ok());
    }

    #[test]
    fn test_validate_attributes_display_type_too_long_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, "value"),
            display_type: Some(String::from_str(&env, &"d".repeat(65))),
        });
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_attributes_display_type_empty_string_fails() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, "value"),
            display_type: Some(String::from_str(&env, "")),
        });
        assert_eq!(validate_attributes(&attrs), Err(Error::InvalidURI));
    }

    // ========== validate_metadata_image tests ==========

    #[test]
    fn test_validate_metadata_image_valid_https() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, "image/jpeg"),
            width: 640,
            height: 480,
        };
        assert!(validate_metadata_image(&env, &image).is_ok());
    }

    #[test]
    fn test_validate_metadata_image_valid_ipfs() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "ipfs://QmThumbHash"),
            mime_type: String::from_str(&env, "image/png"),
            width: 1280,
            height: 720,
        };
        assert!(validate_metadata_image(&env, &image).is_ok());
    }

    #[test]
    fn test_validate_metadata_image_valid_arweave() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "ar://thumb_tx_id"),
            mime_type: String::from_str(&env, "image/webp"),
            width: 800,
            height: 600,
        };
        assert!(validate_metadata_image(&env, &image).is_ok());
    }

    #[test]
    fn test_validate_metadata_image_zero_dimensions_ok() {
        // width/height are unconstrained — 0 is valid for placeholders
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, "image/png"),
            width: 0,
            height: 0,
        };
        assert!(validate_metadata_image(&env, &image).is_ok());
    }

    #[test]
    fn test_validate_metadata_image_empty_url_fails() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, ""),
            mime_type: String::from_str(&env, "image/png"),
            width: 640,
            height: 480,
        };
        assert_eq!(validate_metadata_image(&env, &image), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_image_url_too_long_fails() {
        let env = Env::default();
        let long_url = String::from_str(&env, &format!("https://example.com/{}", "a".repeat(500)));
        let image = MetadataImage {
            image_url: long_url,
            mime_type: String::from_str(&env, "image/png"),
            width: 640,
            height: 480,
        };
        assert_eq!(validate_metadata_image(&env, &image), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_image_unsupported_protocol_fails() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "ftp://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, "image/png"),
            width: 640,
            height: 480,
        };
        assert!(validate_metadata_image(&env, &image).is_err());
    }

    #[test]
    fn test_validate_metadata_image_empty_mime_type_fails() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, ""),
            width: 640,
            height: 480,
        };
        assert_eq!(validate_metadata_image(&env, &image), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_image_mime_type_too_long_fails() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, &"a".repeat(65)),
            width: 640,
            height: 480,
        };
        assert_eq!(validate_metadata_image(&env, &image), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_image_mime_type_max_length_ok() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, &"a".repeat(64)),
            width: 640,
            height: 480,
        };
        assert!(validate_metadata_image(&env, &image).is_ok());
    }
}
