//! Blacklisted wallet storage.
//!
//! Maintains a set of wallet addresses that are blocked from contract
//! interactions.
//!
//! # Storage
//! Key: `DataKey::Blacklisted(address)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::DataKey;

/// Add `wallet` to the blacklist.
pub fn add_wallet(env: &Env, wallet: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Blacklisted(wallet.clone()), &true);
}

/// Remove `wallet` from the blacklist.
pub fn remove_wallet(env: &Env, wallet: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Blacklisted(wallet.clone()));
}

/// Return `true` if `wallet` is blacklisted.
pub fn is_blacklisted(env: &Env, wallet: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Blacklisted(wallet.clone()))
        .unwrap_or(false)
}

// ── Blacklisted clip storage (issue #1176) ───────────────────────────────────
// Maintains blocked clip IDs.
//
// Key: `DataKey::BlacklistedClip(clip_id)` → `bool` (persistent storage)

/// Add `clip_id` to the blocked clip set.
pub fn add_clip(env: &Env, clip_id: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::BlacklistedClip(clip_id), &true);
}

/// Remove `clip_id` from the blocked clip set.
pub fn remove_clip(env: &Env, clip_id: u32) {
    env.storage()
        .persistent()
        .remove(&DataKey::BlacklistedClip(clip_id));
}

/// Return `true` if `clip_id` is blocked.
pub fn is_clip_blacklisted(env: &Env, clip_id: u32) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::BlacklistedClip(clip_id))
        .unwrap_or(false)
}
