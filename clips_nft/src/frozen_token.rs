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

/// Unfreeze a token and report whether a frozen marker was removed.
pub fn unfreeze_token(env: &Env, token_id: TokenId) -> bool {
    let key = DataKey::FrozenToken(token_id);
    if !env.storage().persistent().has(&key) {
        return false;
    }
    env.storage().persistent().remove(&key);
    true
}

/// Return `true` if the token is currently frozen.
pub fn is_frozen(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::FrozenToken(token_id))
        .unwrap_or(false)
}
