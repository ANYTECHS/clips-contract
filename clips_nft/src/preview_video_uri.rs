//! Preview video URI storage — persists and retrieves the short-preview video
//! URI for every minted ClipCash NFT.
//!
//! Resolves issue #669: [Minting] Associate Preview Video URI with NFT.
//!
//! # Storage
//! Key: `DataKey::PreviewVideoUri(token_id)` (persistent storage)
//!
//! # URI validation
//! Accepted schemes: `ipfs://`, `https://`, `ar://` — the same rules applied
//! to the primary metadata URI.  IPFS references are explicitly supported so
//! preview content can be stored in a decentralised, content-addressed way.

use soroban_sdk::{Env, String};

use crate::metadata_uri_builder::validate_uri;
use crate::types::{DataKey, Error, TokenId};

/// Validate and persist the preview video URI for a token.
///
/// # Errors
/// - [`Error::InvalidURI`] — the URI is empty or uses an unsupported scheme.
pub fn set_preview_video_uri(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    validate_uri(uri)?;
    env.storage()
        .persistent()
        .set(&DataKey::PreviewVideoUri(token_id), uri);
    Ok(())
}

/// Retrieve the preview video URI for a token.
///
/// Returns `None` if no preview video URI has been stored for this token.
pub fn get_preview_video_uri(env: &Env, token_id: TokenId) -> Option<String> {
    env.storage()
        .persistent()
        .get(&DataKey::PreviewVideoUri(token_id))
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn set_and_get_ipfs_preview() {
        let env = Env::default();
        let token_id = 1u32;
        let uri = String::from_str(&env, "ipfs://QmPreviewVideo123");

        set_preview_video_uri(&env, token_id, &uri).expect("valid IPFS URI should be accepted");
        assert_eq!(get_preview_video_uri(&env, token_id), Some(uri));
    }

    #[test]
    fn set_and_get_https_preview() {
        let env = Env::default();
        let token_id = 2u32;
        let uri = String::from_str(&env, "https://cdn.example.com/preview.mp4");

        set_preview_video_uri(&env, token_id, &uri).expect("valid HTTPS URI should be accepted");
        assert_eq!(get_preview_video_uri(&env, token_id), Some(uri));
    }

    #[test]
    fn set_and_get_arweave_preview() {
        let env = Env::default();
        let token_id = 3u32;
        let uri = String::from_str(&env, "ar://arweave-preview-tx-id");

        set_preview_video_uri(&env, token_id, &uri).expect("valid Arweave URI should be accepted");
        assert_eq!(get_preview_video_uri(&env, token_id), Some(uri));
    }

    #[test]
    fn rejects_empty_preview_uri() {
        let env = Env::default();
        let token_id = 4u32;
        let uri = String::from_str(&env, "");

        let err =
            set_preview_video_uri(&env, token_id, &uri).expect_err("empty URI should be rejected");
        assert_eq!(err, Error::InvalidURI);
        assert_eq!(get_preview_video_uri(&env, token_id), None);
    }

    #[test]
    fn rejects_unsupported_scheme_preview_uri() {
        let env = Env::default();
        let token_id = 5u32;
        let uri = String::from_str(&env, "ftp://files.example.com/preview.mp4");

        let err = set_preview_video_uri(&env, token_id, &uri)
            .expect_err("unsupported scheme should be rejected");
        assert_eq!(err, Error::InvalidURI);
    }

    #[test]
    fn get_preview_returns_none_when_not_set() {
        let env = Env::default();
        assert_eq!(get_preview_video_uri(&env, 99u32), None);
    }

    #[test]
    fn preview_uri_is_scoped_per_token() {
        let env = Env::default();
        let uri_a = String::from_str(&env, "ipfs://QmPreviewA");
        let uri_b = String::from_str(&env, "ipfs://QmPreviewB");

        set_preview_video_uri(&env, 10, &uri_a).unwrap();
        set_preview_video_uri(&env, 11, &uri_b).unwrap();

        assert_eq!(get_preview_video_uri(&env, 10), Some(uri_a));
        assert_eq!(get_preview_video_uri(&env, 11), Some(uri_b));
        assert_eq!(get_preview_video_uri(&env, 12), None);
    }

    #[test]
    fn preview_uri_can_be_overwritten() {
        let env = Env::default();
        let token_id = 20u32;
        let uri_v1 = String::from_str(&env, "ipfs://QmPreviewV1");
        let uri_v2 = String::from_str(&env, "ipfs://QmPreviewV2");

        set_preview_video_uri(&env, token_id, &uri_v1).unwrap();
        set_preview_video_uri(&env, token_id, &uri_v2).unwrap();

        assert_eq!(get_preview_video_uri(&env, token_id), Some(uri_v2));
    }
}
