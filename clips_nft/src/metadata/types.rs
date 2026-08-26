//! Metadata type definitions.
//!
//! This module contains all core metadata structures used throughout the contract.

use soroban_sdk::{contracttype, Env, String, Vec};

use crate::social_platform::SocialPlatform;

/// Represents an NFT attribute following the OpenSea metadata standard.
///
/// # Fields
/// - `trait_type`: The name of the trait (e.g., "virality_score", "duration")
/// - `value`: The value of the trait (e.g., "98", "42s")
/// - `display_type`: Optional rendering hint that tells marketplaces how to display
///   the value. Follows the OpenSea `display_type` convention. Common values:
///   - `"number"` — numeric value
///   - `"boost_percentage"` — shown as a percentage boost
///   - `"boost_number"` — shown as a raw numeric boost
///   - `"date"` — Unix timestamp rendered as a calendar date
///   - `None` (omitted) — rendered as a plain string
///
/// # Example
/// ```rust,ignore
/// // Plain string attribute (no display_type)
/// let attribute = Attribute {
///     trait_type: String::from_str(&env, "rarity"),
///     value: String::from_str(&env, "legendary"),
///     display_type: None,
/// };
///
/// // Numeric attribute with display hint
/// let score = Attribute {
///     trait_type: String::from_str(&env, "virality_score"),
///     value: String::from_str(&env, "98"),
///     display_type: Some(String::from_str(&env, "number")),
/// };
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    /// The name/type of the attribute (e.g., "Background", "Rarity")
    pub trait_type: String,
    /// The value of the attribute (e.g., "Blue", "Legendary")
    pub value: String,
    /// Optional display hint for marketplaces (e.g., "number", "boost_percentage",
    /// "boost_number", "date"). When `None` the value is rendered as a plain string.
    pub display_type: Option<String>,
}

impl Attribute {
    /// Creates a plain string `Attribute` with no `display_type`.
    ///
    /// Use this when the value should be rendered as a raw string by marketplaces
    /// (e.g., `"rarity": "legendary"`).
    ///
    /// # Arguments
    /// * `trait_type`  – The attribute name (e.g., `"rarity"`, `"platform"`).
    /// * `value`       – The attribute value (e.g., `"legendary"`, `"tiktok"`).
    ///
    /// # Returns
    /// A new `Attribute` with `display_type` set to `None`.
    ///
    /// # Example
    /// ```rust,ignore
    /// let attr = Attribute::new(
    ///     String::from_str(&env, "rarity"),
    ///     String::from_str(&env, "legendary"),
    /// );
    /// assert_eq!(attr.display_type, None);
    /// ```
    pub fn new(trait_type: String, value: String) -> Self {
        Self {
            trait_type,
            value,
            display_type: None,
        }
    }

    /// Creates an `Attribute` with an explicit `display_type` rendering hint.
    ///
    /// Use this when you want a marketplace to render the value in a specific
    /// way (e.g., as a number, a percentage boost, or a calendar date).
    ///
    /// # Arguments
    /// * `trait_type`   – The attribute name (e.g., `"virality_score"`).
    /// * `value`        – The attribute value (e.g., `"98"`).
    /// * `display_type` – Optional IANA/OpenSea rendering hint (e.g.,
    ///   `Some(String::from_str(&env, "number"))`). Pass `None` to produce the
    ///   same result as [`Attribute::new`].
    ///
    /// # Returns
    /// A new `Attribute` with the given `display_type`.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Numeric display
    /// let score = Attribute::with_display_type(
    ///     String::from_str(&env, "virality_score"),
    ///     String::from_str(&env, "98"),
    ///     Some(String::from_str(&env, "number")),
    /// );
    ///
    /// // Date display (Unix timestamp)
    /// let created = Attribute::with_display_type(
    ///     String::from_str(&env, "created"),
    ///     String::from_str(&env, "1546360800"),
    ///     Some(String::from_str(&env, "date")),
    /// );
    /// ```
    pub fn with_display_type(
        trait_type: String,
        value: String,
        display_type: Option<String>,
    ) -> Self {
        Self {
            trait_type,
            value,
            display_type,
        }
    }

    /// Returns `true` if this attribute carries a `display_type` rendering hint.
    ///
    /// # Example
    /// ```rust,ignore
    /// let plain = Attribute::new(trait, value);
    /// assert!(!plain.has_display_type());
    ///
    /// let typed = Attribute::with_display_type(trait, value, Some(dt));
    /// assert!(typed.has_display_type());
    /// ```
    pub fn has_display_type(&self) -> bool {
        self.display_type.is_some()
    }
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

/// Stores the image (thumbnail) information for an NFT.
///
/// This struct is designed to be embedded inside [`ClipMetadata`] (via the
/// `thumbnail` field) and also used stand-alone wherever a rich image
/// representation is needed.  It follows the OpenSea metadata convention for
/// image assets by pairing a URL with its MIME type and pixel dimensions.
///
/// # Fields
/// - `image_url`: Fully-qualified URL to the image asset (`https://`, `ipfs://`,
///   or `ar://` protocols are accepted).
/// - `mime_type`: IANA media-type string describing the image format
///   (e.g., `"image/png"`, `"image/jpeg"`, `"image/gif"`, `"image/webp"`).
/// - `width`: Image width in pixels (`u32`).
/// - `height`: Image height in pixels (`u32`).
///
/// # Serialization
///
/// Derives `#[contracttype]` so the struct is serialised/deserialised
/// transparently by the Soroban SDK (XDR encoding on-chain).
///
/// # Example
/// ```rust,ignore
/// use soroban_sdk::{Env, String};
///
/// let env = Env::default();
///
/// // Minimal constructor
/// let thumb = MetadataImage::new(
///     &env,
///     String::from_str(&env, "https://example.com/thumb.jpg"),
///     String::from_str(&env, "image/jpeg"),
///     640,
///     480,
/// );
///
/// // Struct literal (all four fields are public)
/// let thumb2 = MetadataImage {
///     image_url:  String::from_str(&env, "ipfs://QmThumb"),
///     mime_type:  String::from_str(&env, "image/png"),
///     width:  1280,
///     height: 720,
/// };
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataImage {
    /// Fully-qualified URL to the image asset.
    ///
    /// Must use one of the supported protocols: `https://`, `ipfs://`, `ar://`.
    /// Maximum length: `MAX_URI_LENGTH` (512) characters.
    pub image_url: String,
    /// IANA media-type string (e.g., `"image/png"`, `"image/jpeg"`).
    ///
    /// Must be non-empty. Maximum length: `MAX_MIME_TYPE_LENGTH` (64) characters.
    pub mime_type: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

impl MetadataImage {
    /// Creates a new `MetadataImage` with all required fields.
    ///
    /// # Arguments
    /// * `env`       – The Soroban environment (used for type construction).
    /// * `image_url` – Fully-qualified image URL (`https://`, `ipfs://`, or `ar://`).
    /// * `mime_type` – IANA media-type string (e.g., `"image/png"`).
    /// * `width`     – Image width in pixels.
    /// * `height`    – Image height in pixels.
    ///
    /// # Returns
    /// A new `MetadataImage` instance.
    ///
    /// # Example
    /// ```rust,ignore
    /// let thumb = MetadataImage::new(
    ///     &env,
    ///     String::from_str(&env, "https://example.com/thumb.jpg"),
    ///     String::from_str(&env, "image/jpeg"),
    ///     640,
    ///     480,
    /// );
    /// ```
    pub fn new(
        _env: &soroban_sdk::Env,
        image_url: String,
        mime_type: String,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            image_url,
            mime_type,
            width,
            height,
        }
    }

    /// Returns `true` if the image dimensions are non-zero.
    ///
    /// A `MetadataImage` with zero `width` or zero `height` is considered
    /// dimensionless (e.g., a placeholder created before the real dimensions
    /// are known).
    ///
    /// # Example
    /// ```rust,ignore
    /// let valid = MetadataImage::new(&env, url, mime, 640, 480);
    /// assert!(valid.has_dimensions());
    ///
    /// let placeholder = MetadataImage::new(&env, url, mime, 0, 0);
    /// assert!(!placeholder.has_dimensions());
    /// ```
    pub fn has_dimensions(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Returns the aspect ratio as a `(width, height)` tuple.
    ///
    /// Returns `None` if either dimension is zero to avoid division by zero.
    ///
    /// # Example
    /// ```rust,ignore
    /// let img = MetadataImage::new(&env, url, mime, 1280, 720);
    /// assert_eq!(img.dimensions(), Some((1280, 720)));
    /// ```
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        if self.width == 0 || self.height == 0 {
            None
        } else {
            Some((self.width, self.height))
        }
    }
}

/// ClipMetadata instances are stored in persistent storage using the
/// `DataKey::Metadata(token_id)` key pattern, ensuring long-term availability
/// across contract upgrades.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipMetadata {
    /// Unique identifier for the video clip (must be unique in collection)
    pub clip_id: u32,
    /// Originating social platform
    pub platform: SocialPlatform,
    /// Primary metadata URI (IPFS, Arweave, or HTTPS)
    pub metadata_uri: String,
    /// Creation timestamp (ledger time)
    pub created_at: u64,
    /// Last update timestamp (ledger time)
    pub updated_at: u64,
    /// Optional image preview URL (thumbnail or poster frame)
    pub image: Option<String>,
    /// Optional thumbnail image URL
    pub thumbnail: Option<String>,
    /// Optional animation/video content URL
    pub animation_url: Option<String>,
    /// Optional human-readable description of the clip
    pub description: Option<String>,
    /// Optional external URL for more information
    pub external_url: Option<String>,
    /// Clip duration in seconds (optional)
    pub duration: Option<u64>,
    /// Clip category (e.g., "gaming", "music") (optional)
    pub category: Option<String>,
    /// Language code (e.g., "en", "es") (optional)
    pub language: Option<String>,
        /// AI-generated virality score (optional)
        pub virality_score: Option<u64>,
    /// Array of attributes/traits for the clip
    pub attributes: Vec<Attribute>,
}

/// Creator metadata associated with every NFT.
///
/// Stores creator information including on-chain address, human-readable
/// display name, and a verification flag indicating whether the creator
/// identity has been confirmed by the platform.
///
/// # Fields
/// - `creator_address`: On-chain wallet address of the NFT creator
/// - `display_name`: Human-readable display name for the creator (optional)
/// - `verified`: Flag indicating if the creator has been verified by the platform
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorMetadata {
    /// On-chain wallet address of the NFT creator.
    pub creator_address: soroban_sdk::Address,
    /// Optional human-readable display name for the creator.
    pub display_name: Option<String>,
    /// Flag indicating whether the creator has been verified by the platform.
    /// False by default; set to true only after platform verification.
    pub verified: bool,
}

impl CreatorMetadata {
    /// Creates a new CreatorMetadata with the given creator address.
    ///
    /// Display name is initialized to None and verified is false.
    pub fn new(creator_address: soroban_sdk::Address) -> Self {
        Self {
            creator_address,
            display_name: None,
            verified: false,
        }
    }

    /// Creates a CreatorMetadata with all fields specified.
    pub fn with_details(
        creator_address: soroban_sdk::Address,
        display_name: Option<String>,
        verified: bool,
    ) -> Self {
        Self {
            creator_address,
            display_name,
            verified,
        }
    }

    /// Sets the display name for this creator.
    pub fn set_display_name(mut self, display_name: Option<String>) -> Self {
        self.display_name = display_name;
        self
    }

    /// Sets the verification status for this creator.
    pub fn set_verified(mut self, verified: bool) -> Self {
        self.verified = verified;
        self
    }
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
            platform: SocialPlatform::TikTok, // default platform
            metadata_uri,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
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
        env: &Env,
        clip_id: u32,
        platform: SocialPlatform,
        metadata_uri: String,
        image: Option<String>,
        animation_url: Option<String>,
        description: Option<String>,
        external_url: Option<String>,
        attributes: Vec<Attribute>,
    ) -> Self {
        Self {
            clip_id,
            platform: SocialPlatform::TikTok, // default platform
            metadata_uri,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
            image,
            thumbnail: None,
            animation_url,
            description,
            external_url,
                        duration: None,
            virality_score: None,
            category: None,
            language: None,
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
            || self.thumbnail.is_some()
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

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn updated_at(&self) -> u64 {
        self.updated_at
    }

    pub fn duration(&self) -> Option<u64> {
        self.duration
    }

    pub fn category(&self) -> Option<&String> {
        self.category.as_ref()
    }

    pub fn virality_score(&self) -> Option<u64> {
        self.virality_score
    }

    /// Sets the virality score with validation (0-100 inclusive)
    pub fn set_virality_score(&mut self, score: u64) -> Result<(), &'static str> {
        if score > 100 {
            return Err("Virality score must be between 0 and 100");
        }
        self.virality_score = Some(score);
        Ok(())
    }

    /// Update the updated_at timestamp to current ledger time
    pub fn touch(&mut self, env: &Env) {
        self.updated_at = env.ledger().timestamp();
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
    use crate::metadata::metadata_builder::{ClipMetadataBuilder, TokenMetadataBuilder};
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

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
            display_type: None,
        });
        
        let metadata = ClipMetadata::with_full_data(
            &env,
            clip_id,
            SocialPlatform::TikTok,
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
            &env,
            clip_id,
            SocialPlatform::TikTok,
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
            display_type: None,
        });
        let metadata3 = ClipMetadata::with_full_data(
            &env,
            clip_id,
            SocialPlatform::TikTok,
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
                display_type: None,
            });
        }
        let metadata2 = ClipMetadata::with_full_data(
            &env,
            clip_id,
            SocialPlatform::TikTok,
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
            display_type: None,
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
            display_type: None,
        };
        let attr2 = attr1.clone();
        
        assert_eq!(attr1, attr2);
    }

    // ========== MetadataImage tests ==========

    #[test]
    fn test_metadata_image_creation_struct_literal() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, "image/png"),
            width: 640,
            height: 480,
        };

        assert_eq!(image.image_url, String::from_str(&env, "https://example.com/thumb.jpg"));
        assert_eq!(image.mime_type, String::from_str(&env, "image/png"));
        assert_eq!(image.width, 640);
        assert_eq!(image.height, 480);
    }

    #[test]
    fn test_metadata_image_new_constructor() {
        let env = Env::default();
        let url = String::from_str(&env, "ipfs://QmThumbHash");
        let mime = String::from_str(&env, "image/jpeg");

        let image = MetadataImage::new(&env, url.clone(), mime.clone(), 1280, 720);

        assert_eq!(image.image_url, url);
        assert_eq!(image.mime_type, mime);
        assert_eq!(image.width, 1280);
        assert_eq!(image.height, 720);
    }

    #[test]
    fn test_metadata_image_new_arweave_url() {
        let env = Env::default();
        let url = String::from_str(&env, "ar://thumb_tx_abc123");
        let mime = String::from_str(&env, "image/webp");

        let image = MetadataImage::new(&env, url.clone(), mime.clone(), 800, 600);

        assert_eq!(image.image_url, url);
        assert_eq!(image.mime_type, mime);
    }

    #[test]
    fn test_metadata_image_clone_and_eq() {
        let env = Env::default();
        let image1 = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, "image/jpeg"),
            width: 800,
            height: 600,
        };
        let image2 = image1.clone();

        assert_eq!(image1, image2);
    }

    #[test]
    fn test_metadata_image_inequality_different_url() {
        let env = Env::default();
        let img1 = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/a.jpg"),
            String::from_str(&env, "image/jpeg"),
            640, 480,
        );
        let img2 = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/b.jpg"),
            String::from_str(&env, "image/jpeg"),
            640, 480,
        );
        assert_ne!(img1, img2);
    }

    #[test]
    fn test_metadata_image_inequality_different_mime() {
        let env = Env::default();
        let img1 = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/jpeg"),
            640, 480,
        );
        let img2 = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            640, 480,
        );
        assert_ne!(img1, img2);
    }

    #[test]
    fn test_metadata_image_inequality_different_dimensions() {
        let env = Env::default();
        let img1 = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            640, 480,
        );
        let img2 = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            1280, 720,
        );
        assert_ne!(img1, img2);
    }

    #[test]
    fn test_metadata_image_has_dimensions_true() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            640, 480,
        );
        assert!(image.has_dimensions());
    }

    #[test]
    fn test_metadata_image_has_dimensions_false_zero_width() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            0, 480,
        );
        assert!(!image.has_dimensions());
    }

    #[test]
    fn test_metadata_image_has_dimensions_false_zero_height() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            640, 0,
        );
        assert!(!image.has_dimensions());
    }

    #[test]
    fn test_metadata_image_has_dimensions_false_both_zero() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            0, 0,
        );
        assert!(!image.has_dimensions());
    }

    #[test]
    fn test_metadata_image_dimensions_some() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            1280, 720,
        );
        assert_eq!(image.dimensions(), Some((1280, 720)));
    }

    #[test]
    fn test_metadata_image_dimensions_none_zero_width() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            0, 720,
        );
        assert_eq!(image.dimensions(), None);
    }

    #[test]
    fn test_metadata_image_dimensions_none_zero_height() {
        let env = Env::default();
        let image = MetadataImage::new(
            &env,
            String::from_str(&env, "https://example.com/thumb.jpg"),
            String::from_str(&env, "image/png"),
            1280, 0,
        );
        assert_eq!(image.dimensions(), None);
    }

    #[test]
    fn test_metadata_image_serialization_fields_all_four() {
        let env = Env::default();
        let image = MetadataImage {
            image_url: String::from_str(&env, "https://example.com/thumb.jpg"),
            mime_type: String::from_str(&env, "image/png"),
            width: 640,
            height: 480,
        };

        // All four fields accessible — contracttype serialisation derives from them
        assert_eq!(image.image_url, String::from_str(&env, "https://example.com/thumb.jpg"));
        assert_eq!(image.mime_type, String::from_str(&env, "image/png"));
        assert_eq!(image.width, 640);
        assert_eq!(image.height, 480);
    }

    // ========== TokenMetadata tests ==========

    #[test]
    fn test_token_metadata_new_minimal() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTokenHash");

        let metadata = TokenMetadata::new(&env, uri.clone());

        assert_eq!(metadata.metadata_uri, uri);
        assert_eq!(metadata.image, None);
        assert_eq!(metadata.animation_url, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.external_url, None);
        assert_eq!(metadata.attributes.len(), 0);
        assert!(!metadata.has_optional_fields());
    }

    #[test]
    fn test_token_metadata_has_optional_fields() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        // No optional fields
        let metadata1 = TokenMetadata::new(&env, uri.clone());
        assert!(!metadata1.has_optional_fields());

        // With image only
        let metadata2 = TokenMetadata {
            metadata_uri: uri.clone(),
            image: Some(String::from_str(&env, "https://image.jpg")),
            animation_url: None,
            description: None,
            external_url: None,
            attributes: Vec::new(&env),
        };
        assert!(metadata2.has_optional_fields());

        // With attributes only
        let mut attributes = Vec::new(&env);
        attributes.push_back(Attribute {
            trait_type: String::from_str(&env, "type"),
            value: String::from_str(&env, "value"),
            display_type: None,
        });
        let metadata3 = TokenMetadata {
            metadata_uri: uri,
            image: None,
            animation_url: None,
            description: None,
            external_url: None,
            attributes,
        };
        assert!(metadata3.has_optional_fields());
    }

    #[test]
    fn test_token_metadata_clone_and_eq() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmCloneTest");

        let metadata1 = TokenMetadata::new(&env, uri.clone());
        let metadata2 = metadata1.clone();

        assert_eq!(metadata1, metadata2);
        assert_eq!(metadata1.metadata_uri, metadata2.metadata_uri);
    }

    // ========== ClipMetadata URI generation tests ==========

    #[test]
    fn test_clip_metadata_uri_formats() {
        let env = Env::default();
        
        // Test IPFS URI
        let metadata1 = ClipMetadata::new(&env, 1, String::from_str(&env, "ipfs://QmHash123"));
        assert_eq!(metadata1.metadata_uri, String::from_str(&env, "ipfs://QmHash123"));

        // Test HTTPS URI
        let metadata2 = ClipMetadata::new(&env, 2, String::from_str(&env, "https://example.com/metadata.json"));
        assert_eq!(metadata2.metadata_uri, String::from_str(&env, "https://example.com/metadata.json"));

        // Test Arweave URI
        let metadata3 = ClipMetadata::new(&env, 3, String::from_str(&env, "ar://abc123xyz"));
        assert_eq!(metadata3.metadata_uri, String::from_str(&env, "ar://abc123xyz"));
    }

    // ========== ClipMetadata edge cases ==========

    #[test]
    fn test_clip_metadata_with_empty_attributes() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");
        let attrs = Vec::new(&env);

        let metadata = ClipMetadata::with_full_data(
            &env,
            123,
            SocialPlatform::TikTok,
            uri,
            None,
            None,
            None,
            None,
            attrs,
        );

        assert_eq!(metadata.attributes.len(), 0);
        assert!(!metadata.has_optional_fields());
    }

    #[test]
    fn test_clip_metadata_attribute_count_zero() {
        let env = Env::default();
        let metadata = ClipMetadata::new(&env, 123, String::from_str(&env, "ipfs://QmHash"));
        assert_eq!(metadata.attribute_count(), 0);
    }

    #[test]
    fn test_clip_metadata_with_all_optional_fields() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");
        
        let thumbnail = Some(String::from_str(&env, "https://example.com/thumb.jpg"));

        let mut attributes = Vec::new(&env);
        attributes.push_back(Attribute {
            trait_type: String::from_str(&env, "rarity"),
            value: String::from_str(&env, "legendary"),
            display_type: None,
        });

        let mut metadata = ClipMetadata::with_full_data(
            &env,
            123,
            SocialPlatform::TikTok,
            uri.clone(),
            Some(String::from_str(&env, "https://example.com/image.jpg")),
            Some(String::from_str(&env, "ipfs://QmVideo")),
            Some(String::from_str(&env, "Test description")),
            Some(String::from_str(&env, "https://example.com")),
            attributes,
        );
        metadata.thumbnail = thumbnail;

        assert!(metadata.has_optional_fields());
        assert_eq!(metadata.attribute_count(), 1);
        assert!(metadata.image.is_some());
        assert!(metadata.thumbnail.is_some());
        assert!(metadata.animation_url.is_some());
        assert!(metadata.description.is_some());
        assert!(metadata.external_url.is_some());
    }

    // ========== Serialization/Deserialization tests ==========

    #[test]
    fn test_attribute_serialization_fields() {
        let env = Env::default();
        let attr = Attribute {
            trait_type: String::from_str(&env, "virality_score"),
            value: String::from_str(&env, "98"),
            display_type: None,
        };

        // Verify fields can be accessed (serialization via contracttype)
        assert_eq!(attr.trait_type, String::from_str(&env, "virality_score"));
        assert_eq!(attr.value, String::from_str(&env, "98"));
    }

    #[test]
    fn test_attribute_display_type_serialization_fields() {
        let env = Env::default();

        // Without display_type
        let attr_plain = Attribute {
            trait_type: String::from_str(&env, "rarity"),
            value: String::from_str(&env, "legendary"),
            display_type: None,
        };
        assert_eq!(attr_plain.display_type, None);

        // With display_type = "number"
        let attr_numeric = Attribute {
            trait_type: String::from_str(&env, "virality_score"),
            value: String::from_str(&env, "98"),
            display_type: Some(String::from_str(&env, "number")),
        };
        assert_eq!(
            attr_numeric.display_type,
            Some(String::from_str(&env, "number"))
        );

        // With display_type = "date"
        let attr_date = Attribute {
            trait_type: String::from_str(&env, "created"),
            value: String::from_str(&env, "1546360800"),
            display_type: Some(String::from_str(&env, "date")),
        };
        assert_eq!(
            attr_date.display_type,
            Some(String::from_str(&env, "date"))
        );
    }

    #[test]
    fn test_clip_metadata_serialization_fields() {
        let env = Env::default();
        let metadata = ClipMetadata::new(&env, 12345, String::from_str(&env, "ipfs://QmHash"));

        // Verify all fields are accessible (serialization via contracttype)
        assert_eq!(metadata.clip_id, 12345);
        assert_eq!(metadata.metadata_uri, String::from_str(&env, "ipfs://QmHash"));
        assert_eq!(metadata.image, None);
        assert_eq!(metadata.thumbnail, None);
        assert_eq!(metadata.animation_url, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.external_url, None);
        assert_eq!(metadata.attributes.len(), 0);
    }

    #[test]
    fn test_metadata_image_serialization_fields() {
        // Verify contracttype serialization via the new() constructor
        let env = Env::default();
        let url = String::from_str(&env, "ipfs://QmSerialThumb");
        let mime = String::from_str(&env, "image/gif");
        let image = MetadataImage::new(&env, url.clone(), mime.clone(), 320, 240);

        // All fields accessible — contracttype XDR round-trip depends on all four
        assert_eq!(image.image_url, url);
        assert_eq!(image.mime_type, mime);
        assert_eq!(image.width, 320);
        assert_eq!(image.height, 240);
    }

    #[test]
    fn test_token_metadata_serialization_fields() {
        let env = Env::default();
        let metadata = TokenMetadata::new(&env, String::from_str(&env, "ipfs://QmHash"));

        // Verify all fields are accessible (serialization via contracttype)
        assert_eq!(metadata.metadata_uri, String::from_str(&env, "ipfs://QmHash"));
        assert_eq!(metadata.image, None);
        assert_eq!(metadata.animation_url, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.external_url, None);
        assert_eq!(metadata.attributes.len(), 0);
    }

    // ========== Builder URI generation tests ==========

    #[test]
    fn test_builder_generates_correct_uri() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmGeneratedHash");

        let metadata = ClipMetadataBuilder::new(&env, 12345, uri.clone())
            .build()
            .unwrap();

        assert_eq!(metadata.metadata_uri, uri);
    }

    #[test]
    fn test_builder_preserves_uri_with_special_chars() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash/with/path?query=param");

        let metadata = ClipMetadataBuilder::new(&env, 12345, uri.clone())
            .build()
            .unwrap();

        assert_eq!(metadata.metadata_uri, uri);
    }

    // ========== Error handling tests ==========

    #[test]
    fn test_clip_metadata_new_with_empty_uri() {
        let env = Env::default();
        // Note: ClipMetadata::new doesn't validate, it just creates
        // Validation happens in builder or separately
        let metadata = ClipMetadata::new(&env, 123, String::from_str(&env, ""));
        assert_eq!(metadata.metadata_uri, String::from_str(&env, ""));
    }

    #[test]
    fn test_builder_validates_all_url_fields() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        // Test that builder catches invalid URLs in all optional fields
        let result = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_image(Some(String::from_str(&env, "ftp://invalid.com/image.png")))
            .with_animation_url(Some(String::from_str(&env, "http://insecure.com/video.mp4")))
            .with_external_url(Some(String::from_str(&env, "file:///path/to/file")))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_handles_multiple_validation_errors() {
        let env = Env::default();
        let invalid_uri = String::from_str(&env, "invalid://protocol");

        let result = ClipMetadataBuilder::new(&env, 12345, invalid_uri)
            .with_image(Some(String::from_str(&env, "ftp://invalid.com/image.png")))
            .build();

        // Should fail on metadata_uri validation first
        assert!(result.is_err());
    }

    // ========== Integration-style tests ==========

    #[test]
    fn test_clip_metadata_workflow() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmWorkflowTest");

        // Create metadata with builder
        let metadata = ClipMetadataBuilder::new(&env, 1001, uri.clone())
            .with_image(Some(String::from_str(&env, "https://example.com/image.jpg")))
            .with_animation_url(Some(String::from_str(&env, "ipfs://QmVideo")))
            .with_description(Some(String::from_str(&env, "Test clip")))
            .add_attribute(
                String::from_str(&env, "rarity"),
                String::from_str(&env, "legendary"),
            )
            .build()
            .unwrap();

        // Verify all fields
        assert_eq!(metadata.clip_id, 1001);
        assert_eq!(metadata.metadata_uri, uri);
        assert!(metadata.image.is_some());
        assert!(metadata.animation_url.is_some());
        assert!(metadata.description.is_some());
        assert_eq!(metadata.attributes.len(), 1);
        assert!(metadata.has_optional_fields());
    }

    #[test]
    fn test_token_metadata_workflow() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTokenWorkflow");

        // Create token metadata with builder
        let metadata = TokenMetadataBuilder::new(&env, uri.clone())
            .with_image(Some(String::from_str(&env, "https://example.com/image.jpg")))
            .with_description(Some(String::from_str(&env, "Token description")))
            .add_attribute(
                String::from_str(&env, "type"),
                String::from_str(&env, "premium"),
            )
            .build()
            .unwrap();

        // Verify all fields
        assert_eq!(metadata.metadata_uri, uri);
        assert!(metadata.image.is_some());
        assert!(metadata.description.is_some());
        assert_eq!(metadata.attributes.len(), 1);
        assert!(metadata.has_optional_fields());
    }

    #[test]
    fn test_attribute_validation_in_metadata() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmHash");

        // Create attributes with various trait types and values
        let mut attrs = Vec::new(&env);
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "virality_score"),
            value: String::from_str(&env, "98"),
            display_type: Some(String::from_str(&env, "number")),
        });
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "duration"),
            value: String::from_str(&env, "42s"),
            display_type: None,
        });
        attrs.push_back(Attribute {
            trait_type: String::from_str(&env, "platform"),
            value: String::from_str(&env, "tiktok"),
            display_type: None,
        });

        let metadata = ClipMetadataBuilder::new(&env, 12345, uri)
            .with_attributes(attrs.clone())
            .build()
            .unwrap();

        assert_eq!(metadata.attributes.len(), 3);
        assert_eq!(metadata.attribute_count(), 3);
    }

    // ========== CreatorMetadata struct tests ==========

    #[test]
    fn test_creator_metadata_new_minimal() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let meta = CreatorMetadata::new(creator.clone());

        assert_eq!(meta.creator_address, creator);
        assert_eq!(meta.display_name, None);
        assert!(!meta.verified);
    }

    #[test]
    fn test_creator_metadata_with_details() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = Some(String::from_str(&env, "Alice"));

        let meta = CreatorMetadata::with_details(creator.clone(), name.clone(), true);

        assert_eq!(meta.creator_address, creator);
        assert_eq!(meta.display_name, name);
        assert!(meta.verified);
    }

    #[test]
    fn test_creator_metadata_with_details_no_name_unverified() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let meta = CreatorMetadata::with_details(creator.clone(), None, false);

        assert_eq!(meta.creator_address, creator);
        assert_eq!(meta.display_name, None);
        assert!(!meta.verified);
    }

    #[test]
    fn test_creator_metadata_set_display_name() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = Some(String::from_str(&env, "Bob"));

        let meta = CreatorMetadata::new(creator.clone())
            .set_display_name(name.clone());

        assert_eq!(meta.creator_address, creator);
        assert_eq!(meta.display_name, name);
        assert!(!meta.verified);
    }

    #[test]
    fn test_creator_metadata_set_display_name_to_none() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let meta = CreatorMetadata::with_details(
            creator.clone(),
            Some(String::from_str(&env, "Temp")),
            true,
        )
        .set_display_name(None);

        assert_eq!(meta.display_name, None);
        assert!(meta.verified);
    }

    #[test]
    fn test_creator_metadata_set_verified_true() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let meta = CreatorMetadata::new(creator.clone())
            .set_verified(true);

        assert_eq!(meta.creator_address, creator);
        assert!(meta.verified);
    }

    #[test]
    fn test_creator_metadata_set_verified_false() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let meta = CreatorMetadata::with_details(creator.clone(), None, true)
            .set_verified(false);

        assert!(!meta.verified);
    }

    #[test]
    fn test_creator_metadata_chained_builders() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = Some(String::from_str(&env, "Charlie"));

        let meta = CreatorMetadata::new(creator.clone())
            .set_display_name(name.clone())
            .set_verified(true);

        assert_eq!(meta.creator_address, creator);
        assert_eq!(meta.display_name, name);
        assert!(meta.verified);
    }

    #[test]
    fn test_creator_metadata_clone_and_eq() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = Some(String::from_str(&env, "Dave"));

        let meta1 = CreatorMetadata::with_details(creator.clone(), name.clone(), true);
        let meta2 = meta1.clone();

        assert_eq!(meta1, meta2);
        assert_eq!(meta1.creator_address, meta2.creator_address);
        assert_eq!(meta1.display_name, meta2.display_name);
        assert_eq!(meta1.verified, meta2.verified);
    }

    #[test]
    fn test_creator_metadata_inequality_different_address() {
        let env = Env::default();
        let a = Address::generate(&env);
        let b = Address::generate(&env);

        let meta1 = CreatorMetadata::new(a);
        let meta2 = CreatorMetadata::new(b);

        assert_ne!(meta1, meta2);
    }

    #[test]
    fn test_creator_metadata_inequality_different_name() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let meta1 = CreatorMetadata::with_details(
            creator.clone(),
            Some(String::from_str(&env, "A")),
            false,
        );
        let meta2 = CreatorMetadata::with_details(
            creator.clone(),
            Some(String::from_str(&env, "B")),
            false,
        );

        assert_ne!(meta1, meta2);
    }

    #[test]
    fn test_creator_metadata_inequality_different_verified() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = Some(String::from_str(&env, "Same"));

        let meta1 = CreatorMetadata::with_details(creator.clone(), name.clone(), true);
        let meta2 = CreatorMetadata::with_details(creator.clone(), name.clone(), false);

        assert_ne!(meta1, meta2);
    }

    #[test]
    fn test_creator_metadata_struct_fields() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = Some(String::from_str(&env, "StructTest"));

        let meta = CreatorMetadata {
            creator_address: creator.clone(),
            display_name: name.clone(),
            verified: true,
        };

        assert_eq!(meta.creator_address, creator);
        assert_eq!(meta.display_name, name);
        assert!(meta.verified);
    }
}
