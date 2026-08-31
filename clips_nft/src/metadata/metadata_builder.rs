//! Metadata builder utility.
//!
//! This module provides a fluent builder API for constructing metadata objects
//! with integrated validation and serialization support.
//!
//! ## Features
//!
//! - **Fluent Builder API**: Chainable methods for setting optional fields
//! - **Validation**: Built-in validation of all fields during build
//! - **Serialization**: JSON serialization support for metadata objects
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::metadata::ClipMetadataBuilder;
//!
//! // Create a builder with required fields
//! let builder = ClipMetadataBuilder::new(&env, clip_id, metadata_uri);
//!
//! // Build with optional fields using fluent API
//! let metadata = builder
//!     .with_image(Some(image_url))
//!     .with_animation_url(Some(animation_url))
//!     .with_description(Some(description))
//!     .with_external_url(Some(external_url))
//!     .with_thumbnail(Some(thumbnail))
//!     .with_attributes(attributes)
//!     .build()?;
//! ```

use soroban_sdk::{Env, String, Vec};

use crate::metadata::types::{Attribute, ClipMetadata};
use crate::metadata::helpers::{
    clear_optional_field, filter_empty_attributes, has_duplicate_traits,
};
use crate::metadata::validation::{
    validate_animation_url, validate_attributes, validate_description, validate_external_url,
    validate_image_url, validate_metadata_uri, validate_url,
};
use crate::social_platform::SocialPlatform;
use alloc::format;
use alloc::string::ToString;

/// Builder for constructing ClipMetadata objects with a fluent API.
///
/// This builder provides a convenient way to create metadata instances
/// with optional fields, automatic validation, and serialization support.
///
/// # Example
///
/// ```rust,ignore
/// let metadata = ClipMetadataBuilder::new(&env, 12345, metadata_uri)
///     .with_image(Some(image_url))
///     .with_description(Some(description))
///     .build()?;
/// ```
pub struct ClipMetadataBuilder<'a> {
    env: &'a Env,
    clip_id: u32,
    metadata_uri: String,
    image: Option<String>,
    thumbnail: Option<String>,
    animation_url: Option<String>,
    description: Option<String>,
    external_url: Option<String>,
    duration: Option<u64>,
    category: Option<String>,
    language: Option<String>,
    virality_score: Option<u64>,
    attributes: Vec<Attribute>,
}

impl<'a> ClipMetadataBuilder<'a> {
    /// Creates a new ClipMetadataBuilder with required fields.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `clip_id` - Unique identifier for the video clip
    /// * `metadata_uri` - Primary metadata URI (required)
    ///
    /// # Returns
    /// A new builder instance
    ///
    /// # Example
    /// ```rust,ignore
    /// let builder = ClipMetadataBuilder::new(&env, 12345, String::from_str(&env, "ipfs://QmHash..."));
    /// ```
    pub fn new(env: &'a Env, clip_id: u32, metadata_uri: String) -> Self {
        Self {
            env,
            clip_id,
            metadata_uri,
            image: None,
            thumbnail: None,
            animation_url: None,
            description: None,
            external_url: None,
            duration: None,
            category: None,
            language: None,
            virality_score: None,
            attributes: Vec::new(env),
        }
    }

    /// Sets the image URL (optional).
    ///
    /// # Arguments
    /// * `image` - Optional image preview URL
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.with_image(Some(String::from_str(&env, "https://example.com/image.jpg")))
    /// ```
    pub fn with_image(mut self, image: Option<String>) -> Self {
        self.image = clear_optional_field(&image);
        self
    }

    /// Sets the thumbnail metadata (optional).
    ///
    /// # Arguments
    /// * `thumbnail` - Optional thumbnail metadata
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// let thumbnail = MetadataImage {
    ///     image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
    ///     mime_type: String::from_str(&env, "image/png"),
    ///     width: 640,
    ///     height: 480,
    /// };
    /// builder.with_thumbnail(Some(thumbnail))
    /// ```
    pub fn with_thumbnail(mut self, thumbnail: Option<String>) -> Self {
        self.thumbnail = thumbnail;
        self
    }

    /// Sets the animation URL (optional).
    ///
    /// # Arguments
    /// * `animation_url` - Optional animation/video URL
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.with_animation_url(Some(String::from_str(&env, "ipfs://QmVideo...")))
    /// ```
    pub fn with_animation_url(mut self, animation_url: Option<String>) -> Self {
        self.animation_url = clear_optional_field(&animation_url);
        self
    }

    /// Sets the description (optional).
    ///
    /// # Arguments
    /// * `description` - Optional description text
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.with_description(Some(String::from_str(&env, "Epic gaming moment")))
    /// ```
    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = clear_optional_field(&description);
        self
    }

    /// Sets the external URL (optional).
    ///
    /// # Arguments
    /// * `external_url` - Optional external link
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.with_external_url(Some(String::from_str(&env, "https://clipcash.com/clip/12345")))
    /// ```
    pub fn with_external_url(mut self, external_url: Option<String>) -> Self {
        self.external_url = clear_optional_field(&external_url);
        self
    }

    /// Sets the attributes vector (optional).
    ///
    /// This method automatically filters out any empty attributes.
    ///
    /// # Arguments
    /// * `attributes` - Vector of attributes
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// let mut attrs = Vec::new(&env);
    /// attrs.push_back(Attribute {
    ///     trait_type: String::from_str(&env, "rarity"),
    ///     value: String::from_str(&env, "legendary"),
    /// });
    /// builder.with_attributes(attrs)
    /// ```
    pub fn with_attributes(mut self, attributes: Vec<Attribute>) -> Self {
        self.attributes = filter_empty_attributes(self.env, &attributes);
        self
    }

    /// Adds a single attribute to the metadata.
    ///
    /// # Arguments
    /// * `trait_type` - The attribute trait type
    /// * `value` - The attribute value
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.add_attribute(
    ///     String::from_str(&env, "rarity"),
    ///     String::from_str(&env, "legendary")
    /// )
    /// ```
    pub fn add_attribute(mut self, trait_type: String, value: String) -> Self {
        let attr = Attribute {
            trait_type: trait_type.clone(),
            value: value.clone(),
            display_type: None,
        };
        self.attributes.push_back(attr);
        self
    }

    /// Adds a single attribute with an optional `display_type` rendering hint.
    ///
    /// `display_type` follows the OpenSea metadata standard and tells
    /// marketplaces how to render the value (e.g., `"number"`,
    /// `"boost_percentage"`, `"boost_number"`, `"date"`).
    ///
    /// # Arguments
    /// * `trait_type`   - The attribute trait type
    /// * `value`        - The attribute value
    /// * `display_type` - Optional rendering hint
    ///
    /// # Returns
    /// Self for method chaining
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.add_attribute_typed(
    ///     String::from_str(&env, "virality_score"),
    ///     String::from_str(&env, "98"),
    ///     Some(String::from_str(&env, "number")),
    /// )
    /// ```
    pub fn add_attribute_typed(
        mut self,
        trait_type: String,
        value: String,
        display_type: Option<String>,
    ) -> Self {
        let attr = Attribute {
            trait_type,
            value,
            display_type,
        };
        self.attributes.push_back(attr);
        self
    }

    /// Validates all fields in the builder.
    ///
    /// This method performs comprehensive validation of all metadata fields
    /// according to the contract's validation rules.
    ///
    /// # Returns
    /// - `Ok(())` if all fields are valid
    /// - `Err(Error)` if validation fails
    ///
    /// # Example
    /// ```rust,ignore
    /// builder.validate()?;
    /// ```
    pub fn validate(&self) -> Result<(), crate::types::Error> {
        // Validate required fields
        validate_metadata_uri(self.env, &self.metadata_uri)?;

        // Validate optional URL fields
        validate_image_url(self.env, &self.image)?;
        validate_animation_url(self.env, &self.animation_url)?;
        validate_external_url(self.env, &self.external_url)?;

        // Validate description
        validate_description(&self.description)?;

        // Validate attributes
        validate_attributes(&self.attributes)?;

        // Check for duplicate traits
        if has_duplicate_traits(&self.attributes) {
            return Err(crate::types::Error::InvalidURI);
        }

        Ok(())
    }

    /// Builds the ClipMetadata instance after validation.
    ///
    /// This method validates all fields and constructs the final ClipMetadata
    /// object if validation passes.
    ///
    /// # Returns
    /// - `Ok(ClipMetadata)` if validation passes
    /// - `Err(Error)` if validation fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let metadata = builder.build()?;
    /// ```
    pub fn build(self) -> Result<ClipMetadata, crate::types::Error> {
        self.validate()?;

        Ok(ClipMetadata {
            clip_id: self.clip_id,
            platform: SocialPlatform::TikTok,
            metadata_uri: self.metadata_uri,
            created_at: self.env.ledger().timestamp(),
            updated_at: self.env.ledger().timestamp(),
            image: self.image,
            thumbnail: self.thumbnail,
            animation_url: self.animation_url,
            description: self.description,
            external_url: self.external_url,
            duration: None,
            category: None,
            language: None,
            virality_score: None,
            attributes: self.attributes,
        })
    }

    /// Builds the ClipMetadata instance without validation.
    ///
    /// # Warning
    /// This method skips validation and should only be used when
    /// you're certain the data is already valid.
    ///
    /// # Returns
    /// A new ClipMetadata instance
    pub fn build_unchecked(self) -> ClipMetadata {
        ClipMetadata {
            clip_id: self.clip_id,
            platform: SocialPlatform::TikTok,
            metadata_uri: self.metadata_uri,
            created_at: self.env.ledger().timestamp(),
            updated_at: self.env.ledger().timestamp(),
            image: self.image,
            thumbnail: self.thumbnail,
            animation_url: self.animation_url,
            description: self.description,
            external_url: self.external_url,
            duration: self.duration,
            category: self.category,
            language: self.language,
            virality_score: self.virality_score,
            attributes: self.attributes,
        }
    }

    /// Serializes the metadata to a JSON string.
    ///
    /// This method builds the metadata and converts it to a JSON representation.
    ///
    /// # Returns
    /// - `Ok(String)` containing the JSON representation
    /// - `Err(Error)` if validation fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let json = builder.to_json()?;
    /// ```
    pub fn to_json(&self) -> Result<String, crate::types::Error> {
        // Validate first
        self.validate()?;

        // Build JSON using native alloc strings, then convert to soroban_sdk::String at the end.
        let mut parts: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();

        // Add clip_id
        parts.push(format_json_field("clip_id", &self.clip_id.to_string()));

        // Add metadata_uri
        parts.push(format_json_field(
            "metadata_uri",
            &format!("{}", self.metadata_uri),
        ));

        // Add optional fields
        if let Some(ref img) = self.image {
            parts.push(format_json_field("image", &format!("{}", img)));
        }
        if let Some(ref anim) = self.animation_url {
            parts.push(format_json_field("animation_url", &format!("{}", anim)));
        }
        if let Some(ref desc) = self.description {
            parts.push(format_json_field("description", &format!("{}", desc)));
        }
        if let Some(ref ext) = self.external_url {
            parts.push(format_json_field("external_url", &format!("{}", ext)));
        }

        // Add attributes array
        if !self.attributes.is_empty() {
            let attrs_json = self.serialize_attributes();
            parts.push(format_json_field("attributes", &attrs_json));
        }

        let inner = parts.join(",");
        let json = format!("{{{}}}", inner);
        Ok(String::from_str(self.env, &json))
    }

    /// Serializes attributes to a JSON array string.
    fn serialize_attributes(&self) -> alloc::string::String {
        let mut attr_parts: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
        for attr in self.attributes.iter() {
            attr_parts.push(format!(
                "{{\"trait_type\":\"{}\",\"value\":\"{}\"}}",
                attr.trait_type, attr.value
            ));
        }
        if attr_parts.is_empty() {
            return alloc::string::String::from("[]");
        }
        format!("[{}]", attr_parts.join(","))
    }

    /// Returns a reference to the environment.
    pub fn env(&self) -> &Env {
        self.env
    }
}

/// Helper function to format a JSON field.
fn format_json_field(key: &str, value: &str) -> alloc::string::String {
    format!("\"{}\":{}", key, value)
}

/// Builder for constructing TokenMetadata objects.
///
/// Similar to ClipMetadataBuilder but for the simpler TokenMetadata structure.
///
/// # Example
///
/// ```rust,ignore
/// let metadata = TokenMetadataBuilder::new(&env, metadata_uri)
///     .with_image(Some(image_url))
///     .with_description(Some(description))
///     .build()?;
/// ```
pub struct TokenMetadataBuilder<'a> {
    env: &'a Env,
    metadata_uri: String,
    image: Option<String>,
    animation_url: Option<String>,
    description: Option<String>,
    external_url: Option<String>,
    attributes: Vec<Attribute>,
}

impl<'a> TokenMetadataBuilder<'a> {
    /// Creates a new TokenMetadataBuilder with required field.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `metadata_uri` - Primary metadata URI (required)
    ///
    /// # Returns
    /// A new builder instance
    pub fn new(env: &'a Env, metadata_uri: String) -> Self {
        Self {
            env,
            metadata_uri,
            image: None,
            animation_url: None,
            description: None,
            external_url: None,
            attributes: Vec::new(env),
        }
    }

    /// Sets the image URL (optional).
    pub fn with_image(mut self, image: Option<String>) -> Self {
        self.image = clear_optional_field(&image);
        self
    }

    /// Sets the animation URL (optional).
    pub fn with_animation_url(mut self, animation_url: Option<String>) -> Self {
        self.animation_url = clear_optional_field(&animation_url);
        self
    }

    /// Sets the description (optional).
    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = clear_optional_field(&description);
        self
    }

    /// Sets the external URL (optional).
    pub fn with_external_url(mut self, external_url: Option<String>) -> Self {
        self.external_url = clear_optional_field(&external_url);
        self
    }

    /// Sets the attributes vector (optional).
    pub fn with_attributes(mut self, attributes: Vec<Attribute>) -> Self {
        self.attributes = filter_empty_attributes(self.env, &attributes);
        self
    }

    /// Adds a single attribute.
    pub fn add_attribute(mut self, trait_type: String, value: String) -> Self {
        let attr = Attribute {
            trait_type: trait_type.clone(),
            value: value.clone(),
            display_type: None,
        };
        self.attributes.push_back(attr);
        self
    }

    /// Adds a single attribute with an optional `display_type` rendering hint.
    ///
    /// # Arguments
    /// * `trait_type`   - The attribute trait type
    /// * `value`        - The attribute value
    /// * `display_type` - Optional rendering hint (e.g., `"number"`, `"date"`)
    pub fn add_attribute_typed(
        mut self,
        trait_type: String,
        value: String,
        display_type: Option<String>,
    ) -> Self {
        let attr = Attribute {
            trait_type,
            value,
            display_type,
        };
        self.attributes.push_back(attr);
        self
    }

    /// Validates all fields.
    pub fn validate(&self) -> Result<(), crate::types::Error> {
        // For TokenMetadata, we validate the metadata_uri
        validate_metadata_uri(self.env, &self.metadata_uri)?;

        // Validate optional URL fields
        if let Some(ref img) = self.image {
            if img.len() > 0 {
                validate_url(self.env, img)?;
            }
        }

        if let Some(ref anim) = self.animation_url {
            if anim.len() > 0 {
                validate_url(self.env, anim)?;
            }
        }

        if let Some(ref ext) = self.external_url {
            if ext.len() > 0 {
                validate_url(self.env, ext)?;
            }
        }

        // Validate description length
        if let Some(ref desc) = self.description {
            if desc.len() > 1000 {
                return Err(crate::types::Error::InvalidURI);
            }
        }

        // Validate attributes
        validate_attributes(&self.attributes)?;

        // Check for duplicate traits
        if has_duplicate_traits(&self.attributes) {
            return Err(crate::types::Error::InvalidURI);
        }

        Ok(())
    }

    /// Builds the TokenMetadata instance after validation.
    pub fn build(self) -> Result<crate::metadata::types::TokenMetadata, crate::types::Error> {
        self.validate()?;

        Ok(crate::metadata::types::TokenMetadata {
            metadata_uri: self.metadata_uri,
            image: self.image,
            animation_url: self.animation_url,
            description: self.description,
            external_url: self.external_url,
            attributes: self.attributes,
        })
    }

    /// Builds without validation.
    pub fn build_unchecked(self) -> crate::metadata::types::TokenMetadata {
        crate::metadata::types::TokenMetadata {
            metadata_uri: self.metadata_uri,
            image: self.image,
            animation_url: self.animation_url,
            description: self.description,
            external_url: self.external_url,
            attributes: self.attributes,
        }
    }

    /// Serializes to JSON.
    pub fn to_json(&self) -> Result<String, crate::types::Error> {
        self.validate()?;

        let mut parts: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();

        parts.push(format_json_field(
            "metadata_uri",
            &format!("{}", self.metadata_uri),
        ));

        if let Some(ref img) = self.image {
            parts.push(format_json_field("image", &format!("{}", img)));
        }
        if let Some(ref anim) = self.animation_url {
            parts.push(format_json_field("animation_url", &format!("{}", anim)));
        }
        if let Some(ref desc) = self.description {
            parts.push(format_json_field("description", &format!("{}", desc)));
        }
        if let Some(ref ext) = self.external_url {
            parts.push(format_json_field("external_url", &format!("{}", ext)));
        }

        if !self.attributes.is_empty() {
            let attrs_json = self.serialize_attributes();
            parts.push(format_json_field("attributes", &attrs_json));
        }

        let inner = parts.join(",");
        let json = format!("{{{}}}", inner);
        Ok(String::from_str(self.env, &json))
    }

    /// Serializes attributes to JSON array.
    fn serialize_attributes(&self) -> alloc::string::String {
        let mut attr_parts: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
        for attr in self.attributes.iter() {
            attr_parts.push(format!(
                "{{\"trait_type\":\"{}\",\"value\":\"{}\"}}",
                attr.trait_type, attr.value
            ));
        }
        if attr_parts.is_empty() {
            return alloc::string::String::from("[]");
        }
        format!("[{}]", attr_parts.join(","))
    }

    /// Returns a reference to the environment.
    pub fn env(&self) -> &Env {
        self.env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn test_clip_metadata_builder_minimal() {
        let env = Env::default();
        let clip_id = 12345u32;
        let uri = String::from_str(&env, "ipfs://QmTestHash");

        let metadata = ClipMetadataBuilder::new(&env, clip_id, uri.clone())
            .build()
            .unwrap();

        assert_eq!(metadata.clip_id, clip_id);
        assert_eq!(metadata.metadata_uri, uri);
        assert_eq!(metadata.image, None);
        assert_eq!(metadata.animation_url, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.external_url, None);
        assert_eq!(metadata.attributes.len(), 0);
    }

    #[test]
    fn test_clip_metadata_builder_full() {
        let env = Env::default();
        let clip_id = 67890u32;
        let uri = String::from_str(&env, "ipfs://QmFullHash");
        let image = Some(String::from_str(&env, "https://example.com/image.jpg"));
        let animation = Some(String::from_str(&env, "ipfs://QmVideoHash"));
        let desc = Some(String::from_str(&env, "Epic gaming moment"));
        let external = Some(String::from_str(&env, "https://clipcash.com/clip/67890"));

        let mut attributes = Vec::new(&env);
        attributes.push_back(Attribute {
            trait_type: String::from_str(&env, "rarity"),
            value: String::from_str(&env, "legendary"),
            display_type: None,
        });

        let metadata = ClipMetadataBuilder::new(&env, clip_id, uri.clone())
            .with_image(image.clone())
            .with_animation_url(animation.clone())
            .with_description(desc.clone())
            .with_external_url(external.clone())
            .with_attributes(attributes.clone())
            .build()
            .unwrap();

        assert_eq!(metadata.clip_id, clip_id);
        assert_eq!(metadata.metadata_uri, uri);
        assert_eq!(metadata.image, image);
        assert_eq!(metadata.animation_url, animation);
        assert_eq!(metadata.description, desc);
        assert_eq!(metadata.external_url, external);
        assert_eq!(metadata.attributes.len(), 1);
    }

    #[test]
    fn test_clip_metadata_builder_add_attribute() {
        let env = Env::default();
        let clip_id = 11111u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = ClipMetadataBuilder::new(&env, clip_id, uri)
            .add_attribute(
                String::from_str(&env, "rarity"),
                String::from_str(&env, "legendary"),
            )
            .add_attribute(
                String::from_str(&env, "duration"),
                String::from_str(&env, "42s"),
            )
            .build()
            .unwrap();

        assert_eq!(metadata.attributes.len(), 2);
    }

    #[test]
    fn test_clip_metadata_builder_validation() {
        let env = Env::default();
        let clip_id = 22222u32;
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let result = ClipMetadataBuilder::new(&env, clip_id, invalid_uri)
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_token_metadata_builder() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTokenHash");

        let metadata = TokenMetadataBuilder::new(&env, uri.clone())
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .with_description(Some(String::from_str(&env, "Test token")))
            .build()
            .unwrap();

        assert_eq!(metadata.metadata_uri, uri);
        assert!(metadata.image.is_some());
        assert!(metadata.description.is_some());
    }

    #[test]
    fn test_builder_serialization() {
        let env = Env::default();
        let clip_id = 33333u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        let json = ClipMetadataBuilder::new(&env, clip_id, uri)
            .with_description(Some(String::from_str(&env, "Test")))
            .to_json()
            .unwrap();

        assert!(json.to_string().contains("clip_id"));
        assert!(json.to_string().contains("description"));
    }

    #[test]
    fn test_builder_chain_methods() {
        let env = Env::default();
        let clip_id = 44444u32;
        let uri = String::from_str(&env, "ipfs://QmChain");

        // Test that methods can be chained in any order
        let metadata = ClipMetadataBuilder::new(&env, clip_id, uri)
            .with_description(Some(String::from_str(&env, "Test")))
            .with_image(Some(String::from_str(&env, "https://example.com/img.jpg")))
            .add_attribute(
                String::from_str(&env, "type"),
                String::from_str(&env, "value"),
            )
            .with_external_url(Some(String::from_str(&env, "https://example.com")))
            .build()
            .unwrap();

        assert!(metadata.description.is_some());
        assert!(metadata.image.is_some());
        assert!(metadata.external_url.is_some());
        assert_eq!(metadata.attributes.len(), 1);
    }

    #[test]
    fn test_builder_empty_string_normalization() {
        let env = Env::default();
        let clip_id = 55555u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        // Empty strings should be normalized to None
        let metadata = ClipMetadataBuilder::new(&env, clip_id, uri)
            .with_image(Some(String::from_str(&env, "")))
            .with_description(Some(String::from_str(&env, "   ")))
            .build()
            .unwrap();

        assert_eq!(metadata.image, None);
        assert_eq!(metadata.description, None);
    }

    // ========== ClipMetadataBuilder validation tests ==========

    #[test]
    fn test_builder_invalid_metadata_uri_fails() {
        let env = Env::default();
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let result = ClipMetadataBuilder::new(&env, 12345, invalid_uri)
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_invalid_image_url_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_image(Some(String::from_str(&env, "ftp://invalid.com/image.png")))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_invalid_animation_url_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_animation_url(Some(String::from_str(
                &env,
                "http://insecure.com/video.mp4",
            )))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_invalid_external_url_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_external_url(Some(String::from_str(&env, "file:///path/to/file")))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_duplicate_traits_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

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

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_attributes(attrs)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_too_many_attributes_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let mut attrs = Vec::new(&env);
        for i in 0..51 {
            attrs.push_back(Attribute {
                trait_type: String::from_str(&env, &format!("trait{}", i)),
                value: String::from_str(&env, "value"),
                display_type: None,
            });
        }

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_attributes(attrs)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_empty_trait_type_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, ""),
            value: String::from_str(&env, "value"),
            display_type: None,
        });

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_attributes(attrs)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_empty_attribute_value_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "trait"),
            value: String::from_str(&env, ""),
            display_type: None,
        });

        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_attributes(attrs)
            .build();

        assert!(result.is_err());
    }

    // ========== ClipMetadataBuilder serialization tests ==========

    #[test]
    fn test_builder_to_json_with_all_fields() {
        let env = Env::default();
        let clip_id = 12345u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        let json = ClipMetadataBuilder::new(&env, clip_id, uri)
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .with_animation_url(Some(String::from_str(&env, "ipfs://QmVideo")))
            .with_description(Some(String::from_str(&env, "Test description")))
            .with_external_url(Some(String::from_str(&env, "https://example.com")))
            .to_json()
            .unwrap();

        let json_str = json.to_string();
        assert!(json_str.contains("clip_id"));
        assert!(json_str.contains("metadata_uri"));
        assert!(json_str.contains("image"));
        assert!(json_str.contains("animation_url"));
        assert!(json_str.contains("description"));
        assert!(json_str.contains("external_url"));
    }

    #[test]
    fn test_builder_to_json_with_attributes() {
        let env = Env::default();
        let clip_id = 12345u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        let json = ClipMetadataBuilder::new(&env, clip_id, uri)
            .add_attribute(
                String::from_str(&env, "rarity"),
                String::from_str(&env, "legendary"),
            )
            .add_attribute(
                String::from_str(&env, "duration"),
                String::from_str(&env, "42s"),
            )
            .to_json()
            .unwrap();

        let json_str = json.to_string();
        assert!(json_str.contains("rarity"));
        assert!(json_str.contains("legendary"));
        assert!(json_str.contains("duration"));
        assert!(json_str.contains("42s"));
    }

    #[test]
    fn test_builder_to_json_without_optional_fields() {
        let env = Env::default();
        let clip_id = 12345u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        let json = ClipMetadataBuilder::new(&env, clip_id, uri)
            .to_json()
            .unwrap();

        let json_str = json.to_string();
        assert!(json_str.contains("clip_id"));
        assert!(json_str.contains("metadata_uri"));
        assert!(!json_str.contains("image"));
        assert!(!json_str.contains("animation_url"));
        assert!(!json_str.contains("description"));
        assert!(!json_str.contains("external_url"));
        assert!(!json_str.contains("attributes"));
    }

    #[test]
    fn test_builder_to_json_validation_fails() {
        let env = Env::default();
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let result = ClipMetadataBuilder::new(&env, 12345, invalid_uri).to_json();

        assert!(result.is_err());
    }

    // ========== ClipMetadataBuilder build_unchecked tests ==========

    #[test]
    fn test_builder_build_unchecked_skips_validation() {
        let env = Env::default();
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let metadata = ClipMetadataBuilder::new(&env, 12345, invalid_uri.clone()).build_unchecked();

        assert_eq!(metadata.metadata_uri, invalid_uri);
    }

    #[test]
    fn test_builder_build_unchecked_preserves_all_fields() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = ClipMetadataBuilder::new(&env, 12345, uri.clone())
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .with_description(Some(String::from_str(&env, "Test")))
            .build_unchecked();

        assert_eq!(metadata.clip_id, 12345);
        assert_eq!(metadata.metadata_uri, uri);
        assert!(metadata.image.is_some());
        assert!(metadata.description.is_some());
    }

    // ========== ClipMetadataBuilder with_thumbnail tests ==========

    #[test]
    fn test_builder_with_thumbnail() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let thumbnail = String::from_str(&env, "https://example.com/thumb.jpg");

        let metadata = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_thumbnail(Some(thumbnail.clone()))
            .build()
            .unwrap();

        assert_eq!(metadata.thumbnail, Some(thumbnail));
    }

    #[test]
    fn test_builder_without_thumbnail() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = ClipMetadataBuilder::new(&env, 12345, uri).build().unwrap();

        assert_eq!(metadata.thumbnail, None);
    }

    // ========== TokenMetadataBuilder tests ==========

    #[test]
    fn test_token_metadata_builder_minimal() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = TokenMetadataBuilder::new(&env, uri.clone())
            .build()
            .unwrap();

        assert_eq!(metadata.metadata_uri, uri);
        assert_eq!(metadata.image, None);
        assert_eq!(metadata.animation_url, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.external_url, None);
        assert_eq!(metadata.attributes.len(), 0);
    }

    #[test]
    fn test_token_metadata_builder_full() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = TokenMetadataBuilder::new(&env, uri.clone())
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .with_animation_url(Some(String::from_str(&env, "ipfs://QmVideo")))
            .with_description(Some(String::from_str(&env, "Test description")))
            .with_external_url(Some(String::from_str(&env, "https://example.com")))
            .build()
            .unwrap();

        assert_eq!(metadata.metadata_uri, uri);
        assert!(metadata.image.is_some());
        assert!(metadata.animation_url.is_some());
        assert!(metadata.description.is_some());
        assert!(metadata.external_url.is_some());
    }

    #[test]
    fn test_token_metadata_builder_add_attribute() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = TokenMetadataBuilder::new(&env, uri)
            .add_attribute(
                String::from_str(&env, "rarity"),
                String::from_str(&env, "legendary"),
            )
            .add_attribute(
                String::from_str(&env, "duration"),
                String::from_str(&env, "42s"),
            )
            .build()
            .unwrap();

        assert_eq!(metadata.attributes.len(), 2);
    }

    #[test]
    fn test_token_metadata_builder_validation_fails() {
        let env = Env::default();
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let result = TokenMetadataBuilder::new(&env, invalid_uri)
            .with_image(Some(String::from_str(
                &env,
                "https://example.com/image.jpg",
            )))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_token_metadata_builder_duplicate_traits_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

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

        let result = TokenMetadataBuilder::new(&env, uri)
            .with_attributes(attrs)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_token_metadata_builder_to_json() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let json = TokenMetadataBuilder::new(&env, uri)
            .with_description(Some(String::from_str(&env, "Test")))
            .to_json()
            .unwrap();

        let json_str = json.to_string();
        assert!(json_str.contains("metadata_uri"));
        assert!(json_str.contains("description"));
    }

    #[test]
    fn test_token_metadata_builder_build_unchecked() {
        let env = Env::default();
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let metadata = TokenMetadataBuilder::new(&env, invalid_uri.clone()).build_unchecked();

        assert_eq!(metadata.metadata_uri, invalid_uri);
    }

    #[test]
    fn test_token_metadata_builder_empty_string_normalization() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        let metadata = TokenMetadataBuilder::new(&env, uri)
            .with_image(Some(String::from_str(&env, "")))
            .with_description(Some(String::from_str(&env, "   ")))
            .build()
            .unwrap();

        assert_eq!(metadata.image, None);
        assert_eq!(metadata.description, None);
    }

    #[test]
    fn test_token_metadata_builder_filter_empty_attributes() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

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

        let metadata = TokenMetadataBuilder::new(&env, uri)
            .with_attributes(attrs)
            .build()
            .unwrap();

        assert_eq!(metadata.attributes.len(), 1);
        assert_eq!(
            metadata.attributes.get(0).unwrap().trait_type,
            String::from_str(&env, "valid_trait")
        );
    }
}
