//! Token approval storage.
//!
//! Tracks which address is approved to transfer a specific token on behalf of
//! its owner (single-token approval, analogous to ERC-721 `approve`).
//!
//! # Storage
//! Key: `DataKey::Approval(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::DataKey;

/// Persist an approval: `approved` may transfer `token_id`.
pub fn save_approval(env: &Env, token_id: u32, approved: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Approval(token_id), approved);
}

/// Remove any existing approval for `token_id`.
pub fn remove_approval(env: &Env, token_id: u32) {
    env.storage()
        .persistent()
        .remove(&DataKey::Approval(token_id));
}

/// Return the currently approved address for `token_id`, if any.
pub fn get_approval(env: &Env, token_id: u32) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::Approval(token_id))
}
