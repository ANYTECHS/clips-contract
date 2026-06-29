//! Minted clip index — tracks which ClipCash clip IDs have been minted as NFTs.
//!
//! Provides an existence check and enforces that the same clip ID cannot be minted twice.
//!
//! # Storage
//! Key: `DataKey::ClipMinted(clip_id)` → bool (persistent)

use soroban_sdk::Env;

use crate::types::{DataKey, Error};

/// Register `clip_id` in the minted-clip index.
///
/// Returns `Err(ClipAlreadyMinted)` if `clip_id` is already present.
pub fn add_clip(env: &Env, clip_id: u32) -> Result<(), Error> {
    if env.storage().persistent().has(&DataKey::ClipMinted(clip_id)) {
        return Err(Error::ClipAlreadyMinted);
    }
    env.storage().persistent().set(&DataKey::ClipMinted(clip_id), &true);
    Ok(())
}

/// Return `true` if `clip_id` has been added to the minted-clip index.
pub fn clip_exists(env: &Env, clip_id: u32) -> bool {
    env.storage().persistent().has(&DataKey::ClipMinted(clip_id))
}
