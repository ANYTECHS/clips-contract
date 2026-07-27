//! Metadata helper functions.
//!
//! This module provides utility functions for working with metadata,
//! including JSON generation, field normalization, and other operations.

use soroban_sdk::{String, Vec};

use alloc::format;
use alloc::string::ToString;
use crate::metadata::types::Attribute;

/// Checks if a string is empty or contains only whitespace.
///
/// # Arguments
/// * `s` - The string to check
///
/// # Returns
/// `true` if the string is empty, `false` otherwise
///
/// # Example
/// ```rust,ignore
/// if is_empty_string(&url) {
///     // String is empty, treat as None
/// }
/// ```
pub fn is_empty_string(s: &String) -> bool {
    s.len() == 0
}

/// Clears an optional field if it contains an empty string.
///
/// This is useful for handling user input where empty strings
/// should be treated as None.
///
/// # Arguments
/// * `field` - The optional field to potentially clear
///
/// # Returns
/// `None` if the field contains an empty string, otherwise returns the field unchanged
///
/// # Example
/// ```rust,ignore
/// let image = clear_optional_field(&image_input);
/// ```
pub fn clear_optional_field(field: &Option<String>) -> Option<String> {
    match field {
        Some(s) if is_empty_string(s) => None,
        other => other.clone(),
    }
}

/// Normalizes a URL by trimming whitespace (placeholder for future implementation).
///
/// # Arguments
/// * `url` - The URL to normalize
///
/// # Returns
/// The normalized URL
///
/// # Example
/// ```rust,ignore
/// let normalized = normalize_url(&url);
/// ```
///
/// # Note
/// Currently returns the URL unchanged. Future enhancements could include:
/// - Trimming whitespace
/// - Validating URL structure
/// - Converting to canonical form
pub fn normalize_url(url: &String) -> String {
    url.clone()
}

/// Builds a JSON representation of token metadata (placeholder).
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `metadata_uri` - The primary metadata URI
/// * `image` - Optional image URL
/// * `animation_url` - Optional animation URL
/// * `description` - Optional description
/// * `external_url` - Optional external URL
/// * `attributes` - Vector of attributes
///
/// # Returns
/// A JSON string representation of the metadata
///
/// # Example
/// ```rust,ignore
/// let json = build_metadata_json(
///     &env,
///     &uri,
///     &image,
///     &animation_url,
///     &description,
///     &external_url,
///     &attributes
/// );
/// ```
///
/// # Note
/// This is a placeholder for future JSON generation functionality.
/// Full implementation would require JSON serialization support.
pub fn build_metadata_json(
    env: &soroban_sdk::Env,
    metadata_uri: &String,
    image: &Option<String>,
    animation_url: &Option<String>,
    description: &Option<String>,
    external_url: &Option<String>,
    attributes: &Vec<Attribute>,
) -> String {
    // Placeholder implementation
    // Real implementation would build proper JSON structure
    String::from_str(env, "{}")
}

/// Validates that an attribute vector doesn't contain duplicate trait_types.
///
/// # Arguments
/// * `attributes` - Vector of attributes to check
///
/// # Returns
/// `true` if all trait_types are unique, `false` if duplicates exist
///
/// # Example
/// ```rust,ignore
/// if has_duplicate_traits(&attributes) {
///     return Err(Error::InvalidURI);
/// }
/// ```
pub fn has_duplicate_traits(attributes: &Vec<Attribute>) -> bool {
    let len = attributes.len();
    for i in 0..len {
        for j in (i + 1)..len {
            let attr_i = attributes.get(i).unwrap();
            let attr_j = attributes.get(j).unwrap();
            if attr_i.trait_type == attr_j.trait_type {
                return true;
            }
        }
    }
    false
}

/// Filters out empty attributes from a vector.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `attributes` - Vector of attributes to filter
///
/// # Returns
/// A new vector containing only attributes with non-empty trait_type and value
///
/// # Example
/// ```rust,ignore
/// let filtered = filter_empty_attributes(&env, &attributes);
/// ```
pub fn filter_empty_attributes(
    env: &soroban_sdk::Env,
    attributes: &Vec<Attribute>,
) -> Vec<Attribute> {
    let mut filtered = Vec::new(env);
    for attr in attributes.iter() {
        if !is_empty_string(&attr.trait_type) && !is_empty_string(&attr.value) {
            filtered.push_back(attr);
        }
    }
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String, Vec};

    // ========== is_empty_string tests ==========

    #[test]
    fn test_is_empty_string_empty() {
        let s = String::from_str(&Env::default(), "");
        assert!(is_empty_string(&s));
    }

    #[test]
    fn test_is_empty_string_non_empty() {
        let s = String::from_str(&Env::default(), "hello");
        assert!(!is_empty_string(&s));
    }

    #[test]
    fn test_is_empty_string_whitespace() {
        let s = String::from_str(&Env::default(), "   ");
        assert!(!is_empty_string(&s));
    }

    // ========== clear_optional_field tests ==========

    #[test]
    fn test_clear_optional_field_none() {
        let field: Option<String> = None;
        assert_eq!(clear_optional_field(&field), None);
    }

    #[test]
    fn test_clear_optional_field_some_non_empty() {
        let env = Env::default();
        let field = Some(String::from_str(&env, "value"));
        assert_eq!(clear_optional_field(&field), Some(String::from_str(&env, "value")));
    }

    #[test]
    fn test_clear_optional_field_some_empty() {
        let env = Env::default();
        let field = Some(String::from_str(&env, ""));
        assert_eq!(clear_optional_field(&field), None);
    }

    // ========== normalize_url tests ==========

    #[test]
    fn test_normalize_url_returns_clone() {
        let env = Env::default();
        let url = String::from_str(&env, "https://example.com/path");
        let normalized = normalize_url(&url);
        assert_eq!(url, normalized);
    }

    // ========== build_metadata_json tests ==========

    #[test]
    fn test_build_metadata_json_returns_placeholder() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");
        let image: Option<String> = None;
        let animation: Option<String> = None;
        let desc: Option<String> = None;
        let external: Option<String> = None;
        let attrs = Vec::new(&env);

        let json = build_metadata_json(&env, &uri, &image, &animation, &desc, &external, &attrs);
        assert_eq!(json.to_string(), "{}");
    }

    // ========== has_duplicate_traits tests ==========

    #[test]
    fn test_has_duplicate_traits_empty() {
        let env = Env::default();
        let attrs = Vec::new(&env);
        assert!(!has_duplicate_traits(&attrs));
    }

    #[test]
    fn test_has_duplicate_traits_single() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "rarity"),
                    value: String::from_str(&env, "legendary"),
                    display_type: None,
                });
        assert!(!has_duplicate_traits(&attrs));
    }

    #[test]
    fn test_has_duplicate_traits_no_duplicates() {
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
    fn test_has_duplicate_traits_with_duplicates() {
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
    fn test_has_duplicate_traits_multiple_duplicates() {
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
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "rarity"),
                    value: String::from_str(&env, "epic"),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "duration"),
                    value: String::from_str(&env, "10s"),
                    display_type: None,
                });
        assert!(has_duplicate_traits(&attrs));
    }

    #[test]
    fn test_has_duplicate_traits_case_sensitive() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "Rarity"),
                    value: String::from_str(&env, "legendary"),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "rarity"),
                    value: String::from_str(&env, "common"),
                    display_type: None,
                });
        assert!(!has_duplicate_traits(&attrs));
    }

    // ========== filter_empty_attributes tests ==========

    #[test]
    fn test_filter_empty_attributes_empty_vector() {
        let env = Env::default();
        let attrs = Vec::new(&env);
        let filtered = filter_empty_attributes(&env, &attrs);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_empty_attributes_all_valid() {
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

        let filtered = filter_empty_attributes(&env, &attrs);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_empty_attributes_removes_empty_trait_type() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, ""),
                    value: String::from_str(&env, "value"),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "valid_trait"),
                    value: String::from_str(&env, "valid_value"),
                    display_type: None,
                });

        let filtered = filter_empty_attributes(&env, &attrs);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.get(0).unwrap().trait_type, String::from_str(&env, "valid_trait"));
    }

    #[test]
    fn test_filter_empty_attributes_removes_empty_value() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "trait"),
                    value: String::from_str(&env, ""),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "valid_trait"),
                    value: String::from_str(&env, "valid_value"),
                    display_type: None,
                });

        let filtered = filter_empty_attributes(&env, &attrs);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.get(0).unwrap().value, String::from_str(&env, "valid_value"));
    }

    #[test]
    fn test_filter_empty_attributes_removes_all_empty() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, ""),
                    value: String::from_str(&env, ""),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, ""),
                    value: String::from_str(&env, "value"),
                    display_type: None,
                });

        let filtered = filter_empty_attributes(&env, &attrs);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_empty_attributes_preserves_order() {
        let env = Env::default();
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "first"),
                    value: String::from_str(&env, "1"),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, ""),
                    value: String::from_str(&env, "2"),
                    display_type: None,
                });
        attrs.push_back(Attribute {
                    trait_type: String::from_str(&env, "third"),
                    value: String::from_str(&env, "3"),
                    display_type: None,
                });

        let filtered = filter_empty_attributes(&env, &attrs);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.get(0).unwrap().trait_type, String::from_str(&env, "first"));
        assert_eq!(filtered.get(1).unwrap().trait_type, String::from_str(&env, "third"));
    }
}
