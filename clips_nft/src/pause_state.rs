//! Pause state storage.
//!
//! Persists whether the contract is paused.
//!
//! # Storage
//! Key: `DataKey::Paused` (instance storage)

use soroban_sdk::Env;

use crate::types::DataKey;

/// Persist the pause state.
pub fn save_pause_state(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

/// Return the current pause state (`false` if never set).
pub fn get_pause_state(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}
