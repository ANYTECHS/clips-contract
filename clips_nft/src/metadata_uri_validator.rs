//! Metadata URI validator (Issue #561).
//!
//! Validates that a metadata URI uses one of the supported protocols:
//! - `ipfs://`
//! - `https://`
//! - `ar://`  (Arweave)
//!
//! Rejects empty strings and any URI with an unsupported protocol.

use soroban_sdk::String;

use crate::types::Error;

/// Validate a metadata URI.
///
/// Accepted prefixes: `ipfs://`, `https://`, `ar://`
/// Returns `Err(InvalidURI)` for empty or unsupported URIs.
pub fn validate_metadata_uri(uri: &String) -> Result<(), Error> {
    let len = uri.len();
    if len == 0 {
        return Err(Error::InvalidURI);
    }

    // Collect bytes for prefix check (Soroban String is UTF-8 encoded bytes).
    let bytes = uri.to_bytes();

    if has_prefix(&bytes, b"ipfs://")
        || has_prefix(&bytes, b"https://")
        || has_prefix(&bytes, b"ar://")
    {
        return Ok(());
    }

    Err(Error::InvalidURI)
}

/// Returns true if `data` starts with `prefix`.
fn has_prefix(data: &soroban_sdk::Bytes, prefix: &[u8]) -> bool {
    if data.len() < prefix.len() as u32 {
        return false;
    }
    for (i, &expected) in prefix.iter().enumerate() {
        if data.get(i as u32) != Some(expected) {
            return false;
        }
    }
    true
}
