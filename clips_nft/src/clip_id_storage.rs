//! Clip ID storage — bidirectional mapping between NFT token IDs and ClipCash clip IDs.
//!
//! Ensures every token maps to exactly one clip and prevents the same clip ID from
//! being assigned to more than one token.
//!
//! # Storage
//! - `DataKey::TokenClipId(token_id)` → clip_id (persistent)
//! - `DataKey::ClipIdMinted(clip_id)` → token_id (persistent; also used as dedup sentinel)

use soroban_sdk::Env;

use crate::types::{DataKey, Error, TokenId};

/// Associate `clip_id` with `token_id` and record the reverse mapping.
///
/// Returns `Err(ClipAlreadyMinted)` if `clip_id` is already mapped to another token.
pub fn save_clip_id(env: &Env, token_id: TokenId, clip_id: u32) -> Result<(), Error> {
    if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
        return Err(Error::ClipAlreadyMinted);
    }
    env.storage().persistent().set(&DataKey::TokenClipId(token_id), &clip_id);
    env.storage().persistent().set(&DataKey::ClipIdMinted(clip_id), &token_id);
    Ok(())
}

/// Return the clip ID associated with `token_id`. Returns `Err(TokenNotFound)` if absent.
pub fn get_clip_id(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenClipId(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Return `true` if `clip_id` has already been mapped to a token.
pub fn is_clip_mapped(env: &Env, clip_id: u32) -> bool {
    env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id))
}
