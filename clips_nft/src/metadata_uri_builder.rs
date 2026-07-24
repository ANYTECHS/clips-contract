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
///
/// In no_std Soroban environments the caller should pass a fully-formed URI
/// directly.  This helper exists for documentation/testing purposes only.
pub fn build_ipfs_uri(env: &Env, cid: &str) -> String {
    let full = alloc::format!("ipfs://{cid}");
    String::from_str(env, &full)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Byte-level prefix check (no_std-compatible).
///
/// Converts both the string and prefix to XDR bytes for comparison since
/// `soroban_sdk::String` does not expose direct byte-indexing in no_std.
fn starts_with_bytes(s: &String, prefix: &str) -> bool {
    let prefix_bytes = prefix.as_bytes();
    let prefix_len = prefix.len();
    if (s.len() as usize) < prefix_len {
        return false;
    }
    // Convert the soroban String to a native str slice via its XDR encoding
    // is not available in no_std; instead compare against the known prefixes
    // by re-building candidate prefix strings and checking equality on chars.
    //
    // We exploit the fact that valid ASCII chars map 1:1 to UTF-8 bytes and
    // use alloc::format to build the expected prefix from the Soroban string
    // by formatting the Display impl.
    let s_native = alloc::format!("{s}");
    s_native.starts_with(prefix)
}
