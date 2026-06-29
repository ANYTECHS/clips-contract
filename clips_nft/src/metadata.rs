//! Metadata module for ClipCash NFTs.
//!
//! - IPFS metadata generator (issue #556)
//! - Metadata serializer          (issue #557)
//! - Metadata deserializer        (issue #558)
//! - Metadata validator           (issue #559)

use soroban_sdk::{Bytes, Env, String};

// =============================================================================
// Errors
// =============================================================================

/// Errors returned by metadata operations.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MetadataError {
    EmptyTitle,
    EmptyDescription,
    /// URI is empty or doesn't start with "ipfs://" or "https://".
    InvalidUri,
    /// Image field present but invalid scheme.
    InvalidImage,
    EmptyCreator,
    /// Serialised string doesn't contain exactly four `|` separators.
    MalformedData,
}

// =============================================================================
// ClipMetadata struct
// =============================================================================

/// NFT metadata following the OpenSea / IPFS standard.
///
/// On-chain encoding: `title|description|uri|image|creator`
#[derive(Clone)]
pub struct ClipMetadata {
    pub title: String,
    pub description: String,
    pub uri: String,
    pub image: String,
    pub creator: String,
}

// =============================================================================
// Validator  (issue #559)
// =============================================================================

/// Validate a [`ClipMetadata`] before minting.
pub fn validate(env: &Env, m: &ClipMetadata) -> Result<(), MetadataError> {
    if m.title.len() == 0 {
        return Err(MetadataError::EmptyTitle);
    }
    if m.description.len() == 0 {
        return Err(MetadataError::EmptyDescription);
    }
    if !valid_uri(env, &m.uri) {
        return Err(MetadataError::InvalidUri);
    }
    if m.image.len() > 0 && !valid_uri(env, &m.image) {
        return Err(MetadataError::InvalidImage);
    }
    if m.creator.len() == 0 {
        return Err(MetadataError::EmptyCreator);
    }
    Ok(())
}

fn valid_uri(env: &Env, s: &String) -> bool {
    if s.len() == 0 {
        return false;
    }
    let b = s.to_bytes();
    bytes_starts_with(&b, b"ipfs://") || bytes_starts_with(&b, b"https://")
}

fn bytes_starts_with(b: &Bytes, prefix: &[u8]) -> bool {
    if b.len() < prefix.len() as u32 {
        return false;
    }
    for (i, &p) in prefix.iter().enumerate() {
        if b.get(i as u32) != Some(p) {
            return false;
        }
    }
    true
}

// =============================================================================
// Serializer  (issue #557)
// =============================================================================

/// Serialize [`ClipMetadata`] to `title|description|uri|image|creator`.
pub fn serialize(env: &Env, m: &ClipMetadata) -> String {
    let sep = String::from_str(env, "|");
    let mut out = m.title.clone();
    out.push_str(&sep);
    out.push_str(&m.description);
    out.push_str(&sep);
    out.push_str(&m.uri);
    out.push_str(&sep);
    out.push_str(&m.image);
    out.push_str(&sep);
    out.push_str(&m.creator);
    out
}

// =============================================================================
// Deserializer  (issue #558)
// =============================================================================

/// Deserialize a pipe-delimited [`String`] back into [`ClipMetadata`].
///
/// Returns [`MetadataError::MalformedData`] if there aren't exactly 4 `|` chars.
pub fn deserialize(env: &Env, raw: &String) -> Result<ClipMetadata, MetadataError> {
    let b = raw.to_bytes();
    let len = raw.len();

    let mut seps: [u32; 4] = [0; 4];
    let mut count: usize = 0;

    for i in 0..len {
        if b.get(i) == Some(b'|') {
            if count >= 4 {
                return Err(MetadataError::MalformedData);
            }
            seps[count] = i;
            count += 1;
        }
    }
    if count != 4 {
        return Err(MetadataError::MalformedData);
    }

    Ok(ClipMetadata {
        title:       bytes_slice_string(env, &b, 0,            seps[0]),
        description: bytes_slice_string(env, &b, seps[0] + 1,  seps[1]),
        uri:         bytes_slice_string(env, &b, seps[1] + 1,  seps[2]),
        image:       bytes_slice_string(env, &b, seps[2] + 1,  seps[3]),
        creator:     bytes_slice_string(env, &b, seps[3] + 1,  len),
    })
}

fn bytes_slice_string(env: &Env, b: &Bytes, start: u32, end: u32) -> String {
    let mut buf = Bytes::new(env);
    for i in start..end {
        buf.push_back(b.get(i).unwrap_or(0));
    }
    String::from_bytes(env, &buf)
}

// =============================================================================
// IPFS Metadata Generator  (issue #556)
// =============================================================================

/// Generate an IPFS-compatible JSON metadata string for a ClipCash NFT.
///
/// ```json
/// {"name":"…","description":"…","image":"…","animation_url":"…","creator":"…"}
/// ```
pub fn generate_ipfs_metadata(env: &Env, m: &ClipMetadata) -> String {
    let mut out = String::from_str(env, r#"{"name":""#);
    out.push_str(&m.title);
    out.push_str(&String::from_str(env, r#"","description":""#));
    out.push_str(&m.description);
    out.push_str(&String::from_str(env, r#"","image":""#));
    out.push_str(&m.image);
    out.push_str(&String::from_str(env, r#"","animation_url":""#));
    out.push_str(&m.uri);
    out.push_str(&String::from_str(env, r#"","creator":""#));
    out.push_str(&m.creator);
    out.push_str(&String::from_str(env, r#""}"#));
    out
}
