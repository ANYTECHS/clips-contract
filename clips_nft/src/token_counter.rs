//! Token counter storage.
//!
//! Maintains the total number of NFTs minted.
//!
//! # Storage
//! Key: `DataKey::TokenCounter` (instance storage)

use soroban_sdk::Env;

use crate::types::DataKey;

/// Increment the total minted token counter by one, returning the new value.
pub fn increment_token_counter(env: &Env) -> u32 {
    let next = get_token_counter(env) + 1;
    env.storage().instance().set(&DataKey::TokenCounter, &next);
    next
}

/// Return the total number of NFTs minted (defaults to `0`).
pub fn get_token_counter(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::TokenCounter)
        .unwrap_or(0)
}

/// Reset the counter to `0`. Intended for use in tests.
pub fn reset_token_counter(env: &Env) {
    env.storage().instance().set(&DataKey::TokenCounter, &0u32);
}
