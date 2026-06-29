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
