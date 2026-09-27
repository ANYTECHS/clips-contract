//! Clip metadata types.
//!
//! This module provides the primary `ClipMetadata` struct that represents
//! metadata associated with every ClipCash NFT.

use soroban_sdk::{contracttype, String, Vec};

use crate::metadata::types::Attribute;

/// Primary metadata structure for ClipCash NFTs.
///
/// This struct stores all metadata associated with every ClipCash NFT token,
/// providing a comprehensive representation that follows OpenSea metadata
/// conventions.
///
/// # Fields
///
/// ## Required fields
/// - `clip_id`: Unique identifier for the video clip (unique in collection)
/// - `metadata_uri`: Primary URI pointing to the metadata JSON
///   (e.g. `ipfs://...`, `ar://...`, `https://...`)
///
/// ## Optional media fields
/// - `image`: Optional image preview URL (thumbnail or poster frame)
/// - `thumbnail`: Optional thumbnail metadata (URL + MIME type + dimensions)
///
/// ## Optional descriptive fields
/// - `animation_url`: Optional URL to the animation/video content
/// - `description`: Optional human-readable description of the clip
/// - `external_url`: Optional external link for more information
///
/// ## Attributes
/// - `attributes`: Collection of trait/attribute pairs for filtering and display
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipMetadata {
    /// Unique identifier for the video clip.
    pub clip_id: u32,
    /// Primary metadata URI (IPFS, Arweave, or HTTPS).
    pub metadata_uri: String,

    /// Optional image preview URL.
    pub image: Option<String>,
    /// Optional thumbnail metadata URL.
    pub thumbnail: Option<String>,

    /// Optional animation/video content URL.
    pub animation_url: Option<String>,

    /// Optional human-readable description of the clip.
    pub description: Option<String>,

    /// Optional external URL for additional information.
    pub external_url: Option<String>,

    /// Array of attributes/traits.
    pub attributes: Vec<Attribute>,
}

impl ClipMetadata {
    /// Creates a new `ClipMetadata` instance with only the required fields.
    ///
    /// All optional fields are initialized to `None` (and `attributes` to an
    /// empty vector).
    pub fn new(env: &soroban_sdk::Env, clip_id: u32, metadata_uri: String) -> Self {
        Self {
            clip_id,
            metadata_uri,
            image: None,
            thumbnail: None,
            animation_url: None,
            description: None,
            external_url: None,
            attributes: Vec::new(env),
        }
    }

    /// Creates a `ClipMetadata` instance with the full field set.
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
            thumbnail: None,
            animation_url,
            description,
            external_url,
            attributes,
        }
    }

    /// Returns true if any optional fields are populated.
    pub fn has_optional_fields(&self) -> bool {
        self.image.is_some()
            || self.thumbnail.is_some()
            || self.animation_url.is_some()
            || self.description.is_some()
            || self.external_url.is_some()
            || !self.attributes.is_empty()
    }

    /// Returns the number of attributes associated with this metadata.
    pub fn attribute_count(&self) -> u32 {
        self.attributes.len()
    }
}
