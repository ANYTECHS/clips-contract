//! Original video reference — store and retrieve the source video ID and URL.

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Source reference: an off-chain video ID and its canonical URL.
#[derive(Clone)]
pub struct VideoReference {
    pub source_id: u32,
    pub source_url: String,
}

/// Validate that `url` starts with "http" (non-empty, basic sanity check).
fn validate_url(url: &String) -> Result<(), Error> {
    if url.len() == 0 {
        return Err(Error::InvalidURI);
    }
    Ok(())
}

/// Persist the video reference for `token_id`.
pub fn set_video_reference(
    env: &Env,
    token_id: TokenId,
    source_id: u32,
    source_url: String,
) -> Result<(), Error> {
    validate_url(&source_url)?;
    env.storage()
        .persistent()
        .set(&DataKey::VideoSourceId(token_id), &source_id);
    env.storage()
        .persistent()
        .set(&DataKey::VideoSourceUrl(token_id), &source_url);
    Ok(())
}

/// Return the source ID for `token_id`.
pub fn get_source_id(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::VideoSourceId(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Return the source URL for `token_id`.
pub fn get_source_url(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::VideoSourceUrl(token_id))
        .ok_or(Error::TokenNotFound)
}
