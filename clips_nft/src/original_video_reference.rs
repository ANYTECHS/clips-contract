//! Original Video Reference – high‑level API for storing source video info.
//!
//! This module provides a thin wrapper around the lower‑level `video_reference`
//! storage helpers. It validates that the provided URL starts with "http" (or
//! "https") and then persists both the source ID and URL.

use crate::types::{Error, TokenId};
use crate::video_reference::{get_source_id, get_source_url, set_video_reference};
use soroban_sdk::{Env, String};

/// Validate that the URL is non‑empty and begins with "http" or "https".
fn validate_source_url(url: &String) -> Result<(), Error> {
    if url.len() == 0 {
        return Err(Error::InvalidURI);
    }
    // Simple prefix check – sufficient for on‑chain validation.
    let bytes = url.as_bytes();
    if bytes.starts_with(b"http") {
        Ok(())
    } else {
        Err(Error::InvalidURI)
    }
}

/// Store the original video reference for a given token.
///
/// * `source_id` – off‑chain identifier for the source video.
/// * `source_url` – canonical URL of the source video.
pub fn store_original_video_reference(
    env: &Env,
    token_id: TokenId,
    source_id: u32,
    source_url: String,
) -> Result<(), Error> {
    validate_source_url(&source_url)?;
    set_video_reference(env, token_id, source_id, source_url)
}

/// Retrieve the stored original video reference.
pub fn get_original_video_reference(env: &Env, token_id: TokenId) -> Result<(u32, String), Error> {
    let id = get_source_id(env, token_id)?;
    let url = get_source_url(env, token_id)?;
    Ok((id, url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_reference::{get_source_id, get_source_url, set_video_reference};
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    #[test]
    fn test_store_and_get() {
        let env = Env::default();
        let token_id: TokenId = 42;
        let source_id = 7;
        let source_url = String::from_str(&env, "https://example.com/video.mp4");
        store_original_video_reference(&env, token_id, source_id, source_url.clone()).unwrap();
        let (got_id, got_url) = get_original_video_reference(&env, token_id).unwrap();
        assert_eq!(got_id, source_id);
        assert_eq!(got_url, source_url);
    }
}
