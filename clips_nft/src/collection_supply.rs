//! Collection supply storage.
//!
//! Tracks the number of NFTs minted per collection.
//!
//! # Storage
//! Key: `DataKey::CollectionSupply(collection_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::DataKey;

/// Increment the minted supply counter for the given collection by one.
pub fn increment_collection_supply(env: &Env, collection_id: u32) {
    let current = get_collection_supply(env, collection_id);
    env.storage()
        .persistent()
        .set(&DataKey::CollectionSupply(collection_id), &(current + 1));
}

/// Return the current minted supply for a collection (defaults to `0`).
pub fn get_collection_supply(env: &Env, collection_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::CollectionSupply(collection_id))
        .unwrap_or(0)
}
