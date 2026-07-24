//! Thumbnail URI storage — persists and retrieves the thumbnail image URI for
//! every minted ClipCash NFT.
//!
//! Resolves issue #668: [Minting] Associate Thumbnail URI with NFT.
//!
//! # Storage
//! Key: `DataKey::ThumbnailUri(token_id)` (persistent storage)
//!
//! # URI validation
//! Accepted schemes: `ipfs://`, `https://`, `ar://` — identical rules to the
//! main metadata URI so marketplaces can safely render thumbnails.

use soroban_sdk::{Env, String};

use crate::metadata_uri_builder::validate_uri;
use crate::types::{DataKey, Error, TokenId};

/// Validate and persist the thumbnail URI for a token.
///
/// # Errors
/// - [`Error::InvalidURI`] — the URI is empty or uses an unsupported scheme.
pub fn set_thumbnail_uri(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    validate_uri(uri)?;
    env.storage()
        .persistent()
        .set(&DataKey::ThumbnailUri(token_id), uri);
    Ok(())
}

/// Retrieve the thumbnail URI for a token.
///
/// Returns `None` if no thumbnail URI has been stored for this token.
pub fn get_thumbnail_uri(env: &Env, token_id: TokenId) -> Option<String> {
    env.storage()
        .persistent()
        .get(&DataKey::ThumbnailUri(token_id))
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn set_and_get_ipfs_thumbnail() {
        let env = Env::default();
        let token_id = 1u32;
        let uri = String::from_str(&env, "ipfs://QmThumbnailAbc123");

        set_thumbnail_uri(&env, token_id, &uri).expect("valid IPFS URI should be accepted");
        assert_eq!(get_thumbnail_uri(&env, token_id), Some(uri));
    }

    #[test]
    fn set_and_get_https_thumbnail() {
        let env = Env::default();
        let token_id = 2u32;
        let uri = String::from_str(&env, "https://cdn.example.com/thumbnail.jpg");

        set_thumbnail_uri(&env, token_id, &uri).expect("valid HTTPS URI should be accepted");
        assert_eq!(get_thumbnail_uri(&env, token_id), Some(uri));
    }

    #[test]
    fn set_and_get_arweave_thumbnail() {
        let env = Env::default();
        let token_id = 3u32;
        let uri = String::from_str(&env, "ar://arweave-tx-id-thumbnail");

        set_thumbnail_uri(&env, token_id, &uri).expect("valid Arweave URI should be accepted");
        assert_eq!(get_thumbnail_uri(&env, token_id), Some(uri));
    }

    #[test]
    fn rejects_empty_thumbnail_uri() {
        let env = Env::default();
        let token_id = 4u32;
        let uri = String::from_str(&env, "");

        let err = set_thumbnail_uri(&env, token_id, &uri).expect_err("empty URI should be rejected");
        assert_eq!(err, Error::InvalidURI);
        assert_eq!(get_thumbnail_uri(&env, token_id), None);
    }

    #[test]
    fn rejects_unsupported_scheme_thumbnail_uri() {
        let env = Env::default();
        let token_id = 5u32;
        let uri = String::from_str(&env, "ftp://invalid.example.com/thumb.png");

        let err = set_thumbnail_uri(&env, token_id, &uri).expect_err("unsupported scheme should be rejected");
        assert_eq!(err, Error::InvalidURI);
    }

    #[test]
    fn get_thumbnail_returns_none_when_not_set() {
        let env = Env::default();
        let token_id = 99u32;
        assert_eq!(get_thumbnail_uri(&env, token_id), None);
    }

    #[test]
    fn thumbnail_is_scoped_per_token() {
        let env = Env::default();
        let uri_a = String::from_str(&env, "ipfs://QmThumbA");
        let uri_b = String::from_str(&env, "ipfs://QmThumbB");

        set_thumbnail_uri(&env, 10, &uri_a).unwrap();
        set_thumbnail_uri(&env, 11, &uri_b).unwrap();

        assert_eq!(get_thumbnail_uri(&env, 10), Some(uri_a));
        assert_eq!(get_thumbnail_uri(&env, 11), Some(uri_b));
        assert_eq!(get_thumbnail_uri(&env, 12), None);
    }

    #[test]
    fn thumbnail_can_be_overwritten() {
        let env = Env::default();
        let token_id = 20u32;
        let uri_v1 = String::from_str(&env, "ipfs://QmThumbV1");
        let uri_v2 = String::from_str(&env, "ipfs://QmThumbV2");

        set_thumbnail_uri(&env, token_id, &uri_v1).unwrap();
        set_thumbnail_uri(&env, token_id, &uri_v2).unwrap();

        assert_eq!(get_thumbnail_uri(&env, token_id), Some(uri_v2));
    }
}
