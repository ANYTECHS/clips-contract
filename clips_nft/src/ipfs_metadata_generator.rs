//! IPFS Metadata Generator – creates NFT metadata JSON for IPFS storage.
//!
//! This module provides a single helper that builds the metadata JSON using the
//! existing `ClipMetadataBuilder`. The generated JSON complies with the NFT
//! metadata standard and can be uploaded to IPFS.

use crate::metadata::metadata_builder::ClipMetadataBuilder;
use crate::metadata::types::Attribute;
use crate::types::Error;
use soroban_sdk::{Env, String, Vec};

/// Generate IPFS‑compatible metadata JSON for a ClipCash NFT.
///
/// # Arguments
/// * `env` – Soroban environment.
/// * `clip_id` – Unique identifier for the clip.
/// * `metadata_uri` – Base metadata URI (e.g. `ipfs://QmBaseHash`).
/// * `image` – Optional image URL.
/// * `animation_url` – Optional animation/video URL.
/// * `description` – Optional description text.
/// * `external_url` – Optional external link.
/// * `attributes` – Optional vector of NFT attributes.
///
/// # Returns
/// `Ok(String)` containing the serialized JSON representation, or `Err(Error)`
/// if validation fails.
pub fn generate_ipfs_metadata(
    env: &Env,
    clip_id: u32,
    metadata_uri: String,
    image: Option<String>,
    animation_url: Option<String>,
    description: Option<String>,
    external_url: Option<String>,
    attributes: Vec<Attribute>,
) -> Result<String, Error> {
    // Use the fluent builder to construct the metadata object.
    let builder = ClipMetadataBuilder::new(env, clip_id, metadata_uri)
        .with_image(image)
        .with_animation_url(animation_url)
        .with_description(description)
        .with_external_url(external_url)
        .with_attributes(attributes);

    // Convert to JSON; `to_json` validates internally.
    builder.to_json()
}
