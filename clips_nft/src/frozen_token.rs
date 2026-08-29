//! Frozen token storage — tracks which NFTs are frozen.
//!
//! # Storage
//! Key: `DataKey::FrozenToken(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::{DataKey, TokenId};

/// Mark a token as frozen and report whether the frozen marker was newly set.
pub fn freeze_token(env: &Env, token_id: TokenId) -> bool {
    let key = DataKey::FrozenToken(token_id);
    if env.storage().persistent().has(&key) {
        return false;
    }
    env.storage().persistent().set(&key, &true);
    true
}

/// Unfreeze a token and report whether the marker existed.
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
