//! Tests for the metadata module.
//!
//! This module contains unit and integration tests for metadata functionality.

#![cfg(test)]

use alloc::format;

use soroban_sdk::{Env, String, Vec};

use super::*;
use crate::metadata::{
    helpers::*,
    metadata_builder::{ClipMetadataBuilder, TokenMetadataBuilder},
    storage::*,
    types::{Attribute, ClipMetadata, MetadataImage, TokenMetadata},
    validation::*,
};
use crate::social_platform::SocialPlatform;

// ========== Struct Creation Tests ==========

#[test]
fn test_attribute_creation() {
    let env = Env::default();
    let attr = Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "legendary"),
        display_type: None,
    };
    assert_eq!(attr.trait_type, String::from_str(&env, "rarity"));
    assert_eq!(attr.value, String::from_str(&env, "legendary"));
}

#[test]
fn test_clip_metadata_new_minimal() {
    let env = Env::default();
    let metadata = ClipMetadata::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"));

    assert_eq!(metadata.clip_id, 12345);
    assert_eq!(
        metadata.metadata_uri,
        String::from_str(&env, "ipfs://QmHash")
    );
    assert_eq!(metadata.image, None);
    assert_eq!(metadata.thumbnail, None);
    assert_eq!(metadata.animation_url, None);
    assert_eq!(metadata.description, None);
    assert_eq!(metadata.external_url, None);
    assert_eq!(metadata.attributes.len(), 0);
    assert!(!metadata.has_optional_fields());
}

#[test]
fn test_clip_metadata_with_full_data() {
    let env = Env::default();
    let mut attributes = Vec::new(&env);
    attributes.push_back(Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "legendary"),
        display_type: None,
    });

    let metadata = ClipMetadata::with_full_data(
        &env,
        12345,
        SocialPlatform::TikTok,
        String::from_str(&env, "ipfs://QmHash"),
        Some(String::from_str(&env, "https://example.com/image.jpg")),
        Some(String::from_str(&env, "ipfs://QmVideo")),
        Some(String::from_str(&env, "Test description")),
        Some(String::from_str(&env, "https://example.com")),
        attributes,
    );

    assert_eq!(metadata.clip_id, 12345);
    assert!(metadata.image.is_some());
    assert!(metadata.animation_url.is_some());
    assert!(metadata.description.is_some());
    assert!(metadata.external_url.is_some());
    assert_eq!(metadata.attributes.len(), 1);
    assert!(metadata.has_optional_fields());
}

#[test]
fn test_token_metadata_new() {
    let env = Env::default();
    let metadata = TokenMetadata::new(&env, String::from_str(&env, "ipfs://QmHash"));

    assert_eq!(
        metadata.metadata_uri,
        String::from_str(&env, "ipfs://QmHash")
    );
    assert_eq!(metadata.image, None);
    assert_eq!(metadata.attributes.len(), 0);
    assert!(!metadata.has_optional_fields());
}

#[test]
fn test_metadata_image_creation() {
    let env = Env::default();
    let image = MetadataImage {
        image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
        mime_type: String::from_str(&env, "image/png"),
        width: 640,
        height: 480,
    };

    assert_eq!(
        image.image_url,
        String::from_str(&env, "https://example.com/thumb.jpg")
    );
    assert_eq!(image.mime_type, String::from_str(&env, "image/png"));
    assert_eq!(image.width, 640);
    assert_eq!(image.height, 480);
}

// ========== Validation Tests ==========

#[test]
fn test_validate_url_https() {
    let env = Env::default();
    assert!(validate_url(&env, &String::from_str(&env, "https://example.com")).is_ok());
}

#[test]
fn test_validate_url_ipfs() {
    let env = Env::default();
    assert!(validate_url(&env, &String::from_str(&env, "ipfs://QmHash")).is_ok());
}

#[test]
fn test_validate_url_arweave() {
    let env = Env::default();
    assert!(validate_url(&env, &String::from_str(&env, "ar://abc123")).is_ok());
}

#[test]
fn test_validate_url_unsupported_protocol() {
    let env = Env::default();
    assert!(validate_url(&env, &String::from_str(&env, "ftp://example.com")).is_err());
}

#[test]
fn test_validate_url_empty() {
    let env = Env::default();
    assert!(validate_url(&env, &String::from_str(&env, "")).is_err());
}

#[test]
fn test_validate_metadata_uri_valid() {
    let env = Env::default();
    assert!(validate_metadata_uri(&env, &String::from_str(&env, "ipfs://QmHash")).is_ok());
}

#[test]
fn test_validate_metadata_uri_too_long() {
    let env = Env::default();
    let long_uri = String::from_str(&env, &"a".repeat(513));
    assert!(validate_metadata_uri(&env, &long_uri).is_err());
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
fn test_validate_attributes_too_many() {
    let env = Env::default();
    let mut attrs = Vec::new(&env);
    for i in 0..51 {
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, &format!("trait{}", i)),
            value: String::from_str(&env, "value"),
            display_type: None,
        });
    }
    assert!(validate_attributes(&attrs).is_err());
}

#[test]
fn test_validate_attributes_empty_trait_type() {
    let env = Env::default();
    let mut attrs = Vec::new(&env);
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, ""),
        value: String::from_str(&env, "value"),
        display_type: None,
    });
    assert!(validate_attributes(&attrs).is_err());
}

// ========== Builder Pattern Tests ==========

#[test]
fn test_clip_metadata_builder_minimal() {
    let env = Env::default();
    let metadata = ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"))
        .build()
        .unwrap();

    assert_eq!(metadata.clip_id, 12345);
    assert_eq!(metadata.attributes.len(), 0);
}

#[test]
fn test_clip_metadata_builder_with_options() {
    let env = Env::default();
    let metadata = ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"))
        .with_image(Some(String::from_str(
            &env,
            "https://example.com/image.jpg",
        )))
        .with_description(Some(String::from_str(&env, "Test description")))
        .add_attribute(
            String::from_str(&env, "rarity"),
            String::from_str(&env, "legendary"),
        )
        .build()
        .unwrap();

    assert!(metadata.image.is_some());
    assert!(metadata.description.is_some());
    assert_eq!(metadata.attributes.len(), 1);
}

#[test]
fn test_clip_metadata_builder_validation_fails() {
    let env = Env::default();
    let result =
        ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "invalid://protocol")).build();

    assert!(result.is_err());
}

#[test]
fn test_token_metadata_builder() {
    let env = Env::default();
    let metadata = TokenMetadataBuilder::new(&env, String::from_str(&env, "ipfs://QmHash"))
        .with_image(Some(String::from_str(
            &env,
            "https://example.com/image.jpg",
        )))
        .build()
        .unwrap();

    assert!(metadata.image.is_some());
    assert_eq!(
        metadata.metadata_uri,
        String::from_str(&env, "ipfs://QmHash")
    );
}

// ========== Helper Function Tests ==========

#[test]
fn test_is_empty_string() {
    assert!(is_empty_string(&String::from_str(&Env::default(), "")));
    assert!(!is_empty_string(&String::from_str(
        &Env::default(),
        "hello"
    )));
}

#[test]
fn test_clear_optional_field() {
    let env = Env::default();
    assert_eq!(
        clear_optional_field(&Some(String::from_str(&env, ""))),
        None
    );
    assert_eq!(
        clear_optional_field(&Some(String::from_str(&env, "value"))),
        Some(String::from_str(&env, "value"))
    );
    assert_eq!(clear_optional_field(&None), None);
}

#[test]
fn test_has_duplicate_traits() {
    let env = Env::default();
    let mut attrs = Vec::new(&env);
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "legendary"),
        display_type: None,
    });
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "common"),
        display_type: None,
    });

    assert!(has_duplicate_traits(&attrs));
}

#[test]
fn test_has_duplicate_traits_none() {
    let env = Env::default();
    let mut attrs = Vec::new(&env);
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "legendary"),
        display_type: None,
    });
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "duration"),
        value: String::from_str(&env, "42s"),
        display_type: None,
    });

    assert!(!has_duplicate_traits(&attrs));
}

#[test]
fn test_filter_empty_attributes() {
    let env = Env::default();
    let mut attrs = Vec::new(&env);
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, ""),
        value: String::from_str(&env, "value"),
        display_type: None,
    });
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "valid"),
        value: String::from_str(&env, "valid"),
        display_type: None,
    });

    let filtered = filter_empty_attributes(&env, &attrs);
    assert_eq!(filtered.len(), 1);
}

// ========== Storage Tests ==========

#[test]
fn test_save_and_get_metadata() {
    let env = Env::default();
    let token_id = 1u32;
    let uri = String::from_str(&env, "ipfs://QmHash");

    save_metadata(&env, token_id, &uri);
    let retrieved = get_metadata(&env, token_id);

    assert!(retrieved.is_ok());
    assert_eq!(retrieved.unwrap(), uri);
}

#[test]
fn test_get_metadata_not_found() {
    let env = Env::default();
    let result = get_metadata(&env, 999u32);
    assert!(result.is_err());
}

#[test]
fn test_metadata_exists() {
    let env = Env::default();
    let token_id = 1u32;
    let uri = String::from_str(&env, "ipfs://QmHash");

    assert!(!metadata_exists(&env, token_id));
    save_metadata(&env, token_id, &uri);
    assert!(metadata_exists(&env, token_id));
}

#[test]
fn test_update_metadata() {
    let env = Env::default();
    let token_id = 1u32;
    let uri1 = String::from_str(&env, "ipfs://QmOld");
    let uri2 = String::from_str(&env, "ipfs://QmNew");

    save_metadata(&env, token_id, &uri1);
    update_metadata(&env, token_id, &uri2).unwrap();

    let retrieved = get_metadata(&env, token_id).unwrap();
    assert_eq!(retrieved, uri2);
}

#[test]
fn test_remove_metadata() {
    let env = Env::default();
    let token_id = 1u32;
    let uri = String::from_str(&env, "ipfs://QmHash");

    save_metadata(&env, token_id, &uri);
    assert!(metadata_exists(&env, token_id));

    remove_metadata(&env, token_id);
    assert!(!metadata_exists(&env, token_id));
}

// ========== Serialization Tests ==========

#[test]
fn test_attribute_serialization() {
    let env = Env::default();
    let attr = Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "legendary"),
        display_type: None,
    };

    // Verify fields are accessible (contracttype serialization)
    assert_eq!(attr.trait_type, String::from_str(&env, "rarity"));
    assert_eq!(attr.value, String::from_str(&env, "legendary"));
}

#[test]
fn test_clip_metadata_serialization() {
    let env = Env::default();
    let metadata = ClipMetadata::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"));

    // Verify all fields are accessible (contracttype serialization)
    assert_eq!(metadata.clip_id, 12345);
    assert_eq!(
        metadata.metadata_uri,
        String::from_str(&env, "ipfs://QmHash")
    );
    assert_eq!(metadata.image, None);
    assert_eq!(metadata.attributes.len(), 0);
}

// ========== URI Generation Tests ==========

#[test]
fn test_supported_protocols_constant() {
    assert_eq!(SUPPORTED_PROTOCOLS.len(), 3);
    assert!(SUPPORTED_PROTOCOLS.contains(&"https://"));
    assert!(SUPPORTED_PROTOCOLS.contains(&"ipfs://"));
    assert!(SUPPORTED_PROTOCOLS.contains(&"ar://"));
}

#[test]
fn test_validation_constants() {
    assert!(MAX_URI_LENGTH > 0);
    assert!(MAX_DESCRIPTION_LENGTH > 0);
    assert!(MAX_ATTRIBUTES_COUNT > 0);
    assert!(MAX_TRAIT_TYPE_LENGTH > 0);
    assert!(MAX_TRAIT_VALUE_LENGTH > 0);
}

// ========== Error Handling Tests ==========

#[test]
fn test_builder_invalid_uri() {
    let env = Env::default();
    let result =
        ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "invalid://")).build();
    assert!(result.is_err());
}

#[test]
fn test_builder_invalid_image_url() {
    let env = Env::default();
    let result = ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"))
        .with_image(Some(String::from_str(&env, "ftp://invalid.com")))
        .build();
    assert!(result.is_err());
}

#[test]
fn test_builder_duplicate_traits() {
    let env = Env::default();
    let mut attrs = Vec::new(&env);
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "legendary"),
        display_type: None,
    });
    attrs.push_back(Attribute {
        trait_type: String::from_str(&env, "rarity"),
        value: String::from_str(&env, "common"),
        display_type: None,
    });

    let result = ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"))
        .with_attributes(attrs)
        .build();

    assert!(result.is_err());
}

// ========== Integration Tests ==========

#[test]
fn test_metadata_workflow() {
    let env = Env::default();

    // Create metadata with builder
    let metadata = ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"))
        .with_image(Some(String::from_str(
            &env,
            "https://example.com/image.jpg",
        )))
        .with_animation_url(Some(String::from_str(&env, "ipfs://QmVideo")))
        .with_description(Some(String::from_str(&env, "Test clip")))
        .add_attribute(
            String::from_str(&env, "rarity"),
            String::from_str(&env, "legendary"),
        )
        .build()
        .unwrap();

    // Verify all fields
    assert_eq!(metadata.clip_id, 12345);
    assert!(metadata.image.is_some());
    assert!(metadata.animation_url.is_some());
    assert!(metadata.description.is_some());
    assert_eq!(metadata.attributes.len(), 1);
    assert!(metadata.has_optional_fields());
}

#[test]
fn test_metadata_storage_workflow() {
    let env = Env::default();
    let token_id = 1u32;
    let uri = String::from_str(&env, "ipfs://QmHash");

    // Save metadata
    save_metadata(&env, token_id, &uri);

    // Verify it exists
    assert!(metadata_exists(&env, token_id));

    // Retrieve it
    let retrieved = get_metadata(&env, token_id).unwrap();
    assert_eq!(retrieved, uri);

    // Update it
    let new_uri = String::from_str(&env, "ipfs://QmNewHash");
    update_metadata(&env, token_id, &new_uri).unwrap();
    assert_eq!(get_metadata(&env, token_id).unwrap(), new_uri);

    // Remove it
    remove_metadata(&env, token_id);
    assert!(!metadata_exists(&env, token_id));
}
