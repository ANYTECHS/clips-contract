//! Metadata type definitions.
//!
//! This module contains all core metadata structures used throughout the contract.

use soroban_sdk::{contracttype, String, Vec};

use crate::types::TokenId;

/// Represents an NFT attribute following the OpenSea metadata standard.
///
/// # Fields
/// - `trait_type`: The name of the trait (e.g., "virality_score", "duration")
/// - `value`: The value of the trait (e.g., "98", "42s")
///
/// # Example
/// ```rust,ignore
/// let attribute = Attribute {
///     trait_type: String::from_str(&env, "rarity"),
///     value: String::from_str(&env, "legendary"),
/// };
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    /// The name/type of the attribute (e.g., "Background", "Rarity")
    pub trait_type: String,
    /// The value of the attribute (e.g., "Blue", "Legendary")
    pub value: String,
}

/// Primary metadata structure for ClipCash NFTs.
///
/// This struct stores all metadata associated with every ClipCash NFT token,
/// providing a comprehensive representation that follows OpenSea and EIP-721
/// metadata standards while supporting ClipCash-specific requirements.
///
/// # Fields
///
/// ## Required Fields
/// - `clip_id`: Unique identifier for the video clip (must be unique across collection)
/// - `metadata_uri`: Primary URI pointing to the metadata JSON (IPFS, Arweave, or HTTPS)
///
/// ## Optional Media Fields
/// - `image`: Image preview URL for the NFT (typically a thumbnail or poster frame)
/// - `animation_url`: URL to the actual video/animation content
///
/// ## Optional Descriptive Fields
/// - `description`: Human-readable description of the clip
/// - `external_url`: External link for additional information (e.g., original platform)
///
/// ## Attributes
/// - `attributes`: Collection of trait/attribute pairs for filtering and display
///
/// # Standards Compliance
/// - **OpenSea Metadata Standard**: Compatible with OpenSea's expected format
/// - **EIP-721 Metadata JSON Schema**: Follows Ethereum NFT metadata conventions
/// - **Soroban SDK**: Uses `contracttype` for efficient serialization/deserialization
///
/// # Example
/// ```rust,ignore
/// use soroban_sdk::{Env, String, Vec};
///
/// let env = Env::default();
/// let metadata = ClipMetadata {
///     clip_id: 12345,
///     metadata_uri: String::from_str(&env, "ipfs://QmHash..."),
///     image: Some(String::from_str(&env, "https://example.com/thumb.jpg")),
///     animation_url: Some(String::from_str(&env, "ipfs://QmVideo...")),
///     description: Some(String::from_str(&env, "Epic gaming moment")),
///     external_url: Some(String::from_str(&env, "https://clipcash.com/clip/12345")),
///     attributes: Vec::new(&env),
/// };
/// ```
///
/// # Validation
///
/// All fields are subject to validation rules defined in the validation module:
/// - URIs must use supported protocols (https://, ipfs://, ar://)
/// - String lengths are capped (metadata_uri: 512, description: 1000 chars)
/// - Attributes are limited to 50 per token
/// - Empty optional strings are normalized to None
///
/// # Storage
///
/// ClipMetadata instances are stored in persistent storage using the
/// `DataKey::Metadata(token_id)` key pattern, ensuring long-term availability
/// across contract upgrades.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipMetadata {
    /// Unique identifier for the video clip (must be unique in collection)
    pub clip_id: u32,
    /// Primary metadata URI (IPFS, Arweave, or HTTPS)
    pub metadata_uri: String,
    /// Optional image preview URL (thumbnail or poster frame)
    pub image: Option<String>,
    /// Optional animation/video content URL
    pub animation_url: Option<String>,
    /// Optional human-readable description of the clip
    pub description: Option<String>,
    /// Optional external URL for more information
    pub external_url: Option<String>,
    /// Array of attributes/traits for the clip
    pub attributes: Vec<Attribute>,
}

impl ClipMetadata {
    /// Creates a new ClipMetadata with only the required fields.
    ///
    /// This is the minimal constructor that initializes a ClipMetadata instance
    /// with a clip_id and metadata_uri, leaving all optional fields empty.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment reference
    /// * `clip_id` - Unique identifier for the video clip
    /// * `metadata_uri` - Primary metadata URI
    ///
    /// # Returns
    /// A new ClipMetadata instance with empty optional fields
    ///
    /// # Example
    /// ```rust,ignore
    /// let metadata = ClipMetadata::new(
    ///     &env,
    ///     12345,
    ///     String::from_str(&env, "ipfs://QmHash...")
    /// );
    /// ```
    pub fn new(env: &soroban_sdk::Env, clip_id: u32, metadata_uri: String) -> Self {
        Self {
            clip_id,
            metadata_uri,
            image: None,
            animation_url: None,
            description: None,
            external_url: None,
            attributes: Vec::new(env),
        }
    }

    /// Creates a ClipMetadata with all fields specified.
    ///
    /// This is the full constructor for creating a complete metadata instance
    /// with all optional fields populated.
    ///
    /// # Arguments
    /// * `clip_id` - Unique identifier for the video clip
    /// * `metadata_uri` - Primary metadata URI
    /// * `image` - Optional image preview URL
    /// * `animation_url` - Optional animation/video URL
    /// * `description` - Optional description text
    /// * `external_url` - Optional external link
    /// * `attributes` - Vector of attributes
    ///
    /// # Returns
    /// A new ClipMetadata instance with all fields populated
    ///
    /// # Example
    /// ```rust,ignore
    /// let metadata = ClipMetadata::with_full_data(
    ///     12345,
    ///     String::from_str(&env, "ipfs://QmHash..."),
    ///     Some(String::from_str(&env, "https://example.com/image.jpg")),
    ///     Some(String::from_str(&env, "ipfs://QmVideo...")),
    ///     Some(String::from_str(&env, "Epic gaming clip")),
    ///     Some(String::from_str(&env, "https://clipcash.com/clip/12345")),
    ///     attributes_vec
    /// );
    /// ```
    pub fn with_full_data(
        clip_id: u32,
        metadata_uri: String,
        image: Option<String>,
        animation_url: Option<String>,
        description: Option<String>,
        external_url: Option<String>,
        attributes: Vec<Attribute>,
    ) -> Self {
        Self {
            clip_id,
            metadata_uri,
            image,
            animation_url,
            description,
            external_url,
            attributes,
        }
    }

    /// Checks if any optional fields are populated.
    ///
    /// Returns true if at least one optional field (image, animation_url,
    /// description, external_url) is Some, or if attributes vector is non-empty.
    ///
    /// # Returns
    /// `true` if any optional data exists, `false` if only required fields are set
    ///
    /// # Example
    /// ```rust,ignore
    /// if metadata.has_optional_fields() {
    ///     // Process additional metadata
    /// }
    /// ```
    pub fn has_optional_fields(&self) -> bool {
        self.image.is_some()
            || self.animation_url.is_some()
            || self.description.is_some()
            || self.external_url.is_some()
            || !self.attributes.is_empty()
    }

    /// Returns the number of attributes associated with this metadata.
    ///
    /// # Returns
    /// The count of attributes in the attributes vector
    ///
    /// # Example
    /// ```rust,ignore
    /// let attr_count = metadata.attribute_count();
    /// ```
    pub fn attribute_count(&self) -> u32 {
        self.attributes.len()
    }
}

/// Complete metadata representation for an NFT token.
///
/// This structure holds all metadata fields that can be associated with an NFT,
/// following OpenSea and general NFT metadata standards.
///
/// # Fields
/// - `metadata_uri`: Primary metadata URI (typically IPFS or Arweave)
/// - `image`: Optional image URL
/// - `animation_url`: Optional animation/video URL
/// - `description`: Optional text description
/// - `external_url`: Optional external link
/// - `attributes`: Collection of trait attributes
///
/// # Standards Compliance
/// - OpenSea Metadata Standard
/// - EIP-721 Metadata JSON Schema
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenMetadata {
    /// Primary metadata URI (IPFS, Arweave, or HTTPS)
    pub metadata_uri: String,
    /// Optional image URL
    pub image: Option<String>,
    /// Optional animation or video URL
    pub animation_url: Option<String>,
    /// Optional text description of the NFT
    pub description: Option<String>,
    /// Optional external URL for more information
    pub external_url: Option<String>,
    /// Array of attributes/traits
    pub attributes: Vec<Attribute>,
}

impl TokenMetadata {
    /// Creates a new TokenMetadata with only the required metadata_uri field.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `metadata_uri` - The primary metadata URI
    ///
    /// # Returns
    /// A new TokenMetadata instance with empty optional fields
    pub fn new(env: &soroban_sdk::Env, metadata_uri: String) -> Self {
        Self {
            metadata_uri,
            image: None,
            animation_url: None,
            description: None,
            external_url: None,
            attributes: Vec::new(env),
        }
    }

    /// Checks if any optional fields are populated.
    pub fn has_optional_fields(&self) -> bool {
        self.image.is_some()
            || self.animation_url.is_some()
            || self.description.is_some()
            || self.external_url.is_some()
            || !self.attributes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn test_clip_metadata_new_minimal() {
        let env = Env::default();
        let clip_id = 12345u32;
        let uri = String::from_str(&env, "ipfs://QmTestHash");
        
        let metadata = ClipMetadata::new(&env, clip_id, uri.clone());
        
        assert_eq!(metadata.clip_id, clip_id);
        assert_eq!(metadata.metadata_uri, uri);
        assert_eq!(metadata.image, None);
        assert_eq!(metadata.animation_url, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.external_url, None);
        assert_eq!(metadata.attributes.len(), 0);
        assert!(!metadata.has_optional_fields());
    }

    #[test]
    fn test_clip_metadata_with_full_data() {
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
        });
        
        let metadata = ClipMetadata::with_full_data(
            clip_id,
            uri.clone(),
            image.clone(),
            animation.clone(),
            desc.clone(),
            external.clone(),
            attributes.clone(),
        );
        
        assert_eq!(metadata.clip_id, clip_id);
        assert_eq!(metadata.metadata_uri, uri);
        assert_eq!(metadata.image, image);
        assert_eq!(metadata.animation_url, animation);
        assert_eq!(metadata.description, desc);
        assert_eq!(metadata.external_url, external);
        assert_eq!(metadata.attributes.len(), 1);
        assert!(metadata.has_optional_fields());
    }

    #[test]
    fn test_clip_metadata_has_optional_fields() {
        let env = Env::default();
        let clip_id = 111u32;
        let uri = String::from_str(&env, "ipfs://QmHash");
        
        // No optional fields
        let metadata1 = ClipMetadata::new(&env, clip_id, uri.clone());
        assert!(!metadata1.has_optional_fields());
        
        // With image only
        let metadata2 = ClipMetadata::with_full_data(
            clip_id,
            uri.clone(),
            Some(String::from_str(&env, "https://image.jpg")),
            None,
            None,
            None,
            Vec::new(&env),
        );
        assert!(metadata2.has_optional_fields());
        
        // With attributes only
        let mut attributes = Vec::new(&env);
        attributes.push_back(Attribute {
            trait_type: String::from_str(&env, "type"),
            value: String::from_str(&env, "value"),
        });
        let metadata3 = ClipMetadata::with_full_data(
            clip_id,
            uri.clone(),
            None,
            None,
            None,
            None,
            attributes,
        );
        assert!(metadata3.has_optional_fields());
    }

    #[test]
    fn test_clip_metadata_attribute_count() {
        let env = Env::default();
        let clip_id = 222u32;
        let uri = String::from_str(&env, "ipfs://QmHash");
        
        // Zero attributes
        let metadata1 = ClipMetadata::new(&env, clip_id, uri.clone());
        assert_eq!(metadata1.attribute_count(), 0);
        
        // Multiple attributes
        let mut attributes = Vec::new(&env);
        for i in 0..5 {
            attributes.push_back(Attribute {
                trait_type: String::from_str(&env, "trait"),
                value: String::from_str(&env, "value"),
            });
        }
        let metadata2 = ClipMetadata::with_full_data(
            clip_id,
            uri,
            None,
            None,
            None,
            None,
            attributes,
        );
        assert_eq!(metadata2.attribute_count(), 5);
    }

    #[test]
    fn test_clip_metadata_clone_and_eq() {
        let env = Env::default();
        let clip_id = 333u32;
        let uri = String::from_str(&env, "ipfs://QmCloneTest");
        
        let metadata1 = ClipMetadata::new(&env, clip_id, uri.clone());
        let metadata2 = metadata1.clone();
        
        assert_eq!(metadata1, metadata2);
        assert_eq!(metadata1.clip_id, metadata2.clip_id);
        assert_eq!(metadata1.metadata_uri, metadata2.metadata_uri);
    }

    #[test]
    fn test_attribute_creation() {
        let env = Env::default();
        let trait_type = String::from_str(&env, "virality_score");
        let value = String::from_str(&env, "98");
        
        let attribute = Attribute {
            trait_type: trait_type.clone(),
            value: value.clone(),
        };
        
        assert_eq!(attribute.trait_type, trait_type);
        assert_eq!(attribute.value, value);
    }

    #[test]
    fn test_attribute_clone_and_eq() {
        let env = Env::default();
        let attr1 = Attribute {
            trait_type: String::from_str(&env, "duration"),
            value: String::from_str(&env, "42s"),
        };
        let attr2 = attr1.clone();
        
        assert_eq!(attr1, attr2);
    }
}
