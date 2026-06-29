//! Frozen token storage — tracks which NFTs are frozen.
//!
//! # Storage
//! Key: `DataKey::FrozenToken(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::{DataKey, TokenId};

/// Mark a token as frozen.
pub fn freeze_token(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .set(&DataKey::FrozenToken(token_id), &true);
}

/// Unfreeze a token.
pub fn unfreeze_token(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::FrozenToken(token_id));
}

/// Return `true` if the token is currently frozen.
pub fn is_frozen(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::FrozenToken(token_id))
        .unwrap_or(false)
}
