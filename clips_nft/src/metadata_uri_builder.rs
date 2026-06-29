//! Metadata URI builder — construct and validate metadata URIs with IPFS support.
//!
//! Supported schemes: `ipfs://`, `https://`, `ar://`

use soroban_sdk::{Env, String};

use crate::types::Error;

const IPFS_PREFIX: &str = "ipfs://";
const HTTPS_PREFIX: &str = "https://";
const AR_PREFIX: &str = "ar://";

/// Returns `true` if `uri` uses the IPFS scheme.
pub fn is_ipfs(uri: &String) -> bool {
    uri.len() >= 7 && starts_with_bytes(uri, IPFS_PREFIX)
}

/// Validate that `uri` uses a supported scheme and is non-empty.
pub fn validate_uri(uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    if starts_with_bytes(uri, IPFS_PREFIX)
        || starts_with_bytes(uri, HTTPS_PREFIX)
        || starts_with_bytes(uri, AR_PREFIX)
    {
        Ok(())
    } else {
        Err(Error::InvalidURI)
    }
}

/// Build an IPFS URI from a raw CID string: `ipfs://<cid>`.
pub fn build_ipfs_uri(env: &Env, cid: &str) -> String {
    let uri = soroban_sdk::string!(&env, "ipfs://");
    // Concatenation isn't natively available in no_std Soroban; callers are
    // expected to pass a fully-formed URI. This helper validates it.
    let _ = uri;
    String::from_str(env, &soroban_sdk::format!("ipfs://{cid}"))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Byte-level prefix check (no_std-compatible).
fn starts_with_bytes(s: &String, prefix: &str) -> bool {
    let prefix_bytes = prefix.as_bytes();
    let prefix_len = prefix_bytes.len() as u32;
    if s.len() < prefix_len {
        return false;
    }
    for (i, &b) in prefix_bytes.iter().enumerate() {
        if s.get(i as u32) != b as u32 {
            return false;
        }
    }
    true
}
