//! Collection supply storage.
//!
//! Tracks the number of NFTs minted per collection.
//!
//! # Storage
//! Key: [`DataKey::CollectionSupply(u32)`] (persistent storage).

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

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn get_collection_supply_defaults_to_zero() {
        let env = Env::default();
        assert_eq!(get_collection_supply(&env, 1), 0);
    }

    #[test]
    fn increment_collection_supply_increases_by_one() {
        let env = Env::default();
        let collection_id = 1;
        increment_collection_supply(&env, collection_id);
        assert_eq!(get_collection_supply(&env, collection_id), 1);
        increment_collection_supply(&env, collection_id);
        assert_eq!(get_collection_supply(&env, collection_id), 2);
    }

    #[test]
    fn different_collections_have_separate_supplies() {
        let env = Env::default();
        let collection_1 = 1;
        let collection_2 = 2;
        increment_collection_supply(&env, collection_1);
        increment_collection_supply(&env, collection_1);
        assert_eq!(get_collection_supply(&env, collection_1), 2);
        assert_eq!(get_collection_supply(&env, collection_2), 0);
        increment_collection_supply(&env, collection_2);
        assert_eq!(get_collection_supply(&env, collection_2), 1);
    }
}
