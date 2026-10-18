//! Metadata size validator (Issue #560).
//!
//! Enforces a maximum byte length on metadata URIs to prevent unbounded
//! on-chain storage growth.
//!
//! The limit mirrors typical IPFS / Arweave URI lengths and leaves headroom
//! for longer content-addressed URIs while preventing abuse.

use soroban_sdk::String;

use crate::types::Error;

/// Maximum allowed byte length for a metadata URI.
pub const MAX_METADATA_URI_BYTES: u32 = 512;

/// Validate that a metadata URI does not exceed [`MAX_METADATA_URI_BYTES`].
///
/// Returns `Err(MetadataSizeTooLarge)` if the URI is over the limit.
pub fn validate_metadata_size(uri: &String) -> Result<(), Error> {
    if uri.len() > MAX_METADATA_URI_BYTES {
        return Err(Error::MetadataSizeTooLarge);
    }
    Ok(())
}
