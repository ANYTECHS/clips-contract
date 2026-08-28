//! Creator-level royalty configuration storage.
//!
//! Maps a creator wallet address to a `RoyaltyConfig` that defines the
//! default royalty recipient and basis points for clips minted by that creator.
//!
//! Key: `DataKey::CreatorRoyalty(creator)` → `RoyaltyConfig` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::royalty_config::RoyaltyConfig;
use crate::types::{DataKey, Error};

/// Persist a `RoyaltyConfig` for a creator. Overwrites any existing value.
///
/// Validates the config before writing.
pub fn set_creator_royalty(env: &Env, creator: &Address, cfg: &RoyaltyConfig) -> Result<(), Error> {
    cfg.validate()?;
    env.storage()
        .persistent()
        .set(&DataKey::CreatorRoyalty(creator.clone()), cfg);
    Ok(())
}

/// Retrieve a creator's `RoyaltyConfig`.
///
/// Returns `Err(Error::StorageNotFound)` when no config is stored for `creator`.
pub fn get_creator_royalty(env: &Env, creator: &Address) -> Result<RoyaltyConfig, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorRoyalty(creator.clone()))
        .ok_or(Error::StorageNotFound)
}

/// Update an existing creator royalty config.
///
/// Returns `Err(Error::StorageNotFound)` if no prior config exists for `creator`.
pub fn update_creator_royalty(env: &Env, creator: &Address, cfg: &RoyaltyConfig) -> Result<(), Error> {
    // Ensure record exists
    if !env.storage().persistent().has(&DataKey::CreatorRoyalty(creator.clone())) {
        return Err(Error::StorageNotFound);
    }
    // Validate then write
    cfg.validate()?;
    env.storage()
        .persistent()
        .set(&DataKey::CreatorRoyalty(creator.clone()), cfg);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use crate::MAX_ROYALTY_BPS;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn set_and_get_creator_royalty_round_trip() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let cfg = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: 500,
            };
            set_creator_royalty(env, &creator, &cfg).unwrap();
            let loaded = get_creator_royalty(env, &creator).unwrap();
            assert_eq!(loaded.royalty_bps, 500);
            assert_eq!(loaded.recipient, cfg.recipient);
        });
    }

    #[test]
    fn get_creator_royalty_missing_returns_storage_not_found() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert_eq!(get_creator_royalty(env, &creator), Err(Error::StorageNotFound));
        });
    }

    #[test]
    fn update_creator_royalty_succeeds_when_exists() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let initial = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: 250,
            };
            set_creator_royalty(env, &creator, &initial).unwrap();

            let updated = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: 750,
            };
            update_creator_royalty(env, &creator, &updated).unwrap();
            let loaded = get_creator_royalty(env, &creator).unwrap();
            assert_eq!(loaded.royalty_bps, 750);
            assert_eq!(loaded.recipient, updated.recipient);
        });
    }

    #[test]
    fn update_creator_royalty_fails_when_absent() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let cfg = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: 100,
            };
            assert_eq!(update_creator_royalty(env, &creator, &cfg), Err(Error::StorageNotFound));
        });
    }

    #[test]
    fn validate_rejects_above_max_bps_on_set() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let cfg = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: MAX_ROYALTY_BPS + 1,
            };
            assert_eq!(set_creator_royalty(env, &creator, &cfg), Err(Error::RoyaltyTooHigh));
        });
    }

    #[test]
    fn configs_are_isolated_per_creator() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let ca = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: 200,
            };
            let cb = RoyaltyConfig {
                recipient: Address::generate(env),
                royalty_bps: 800,
            };
            set_creator_royalty(env, &a, &ca).unwrap();
            set_creator_royalty(env, &b, &cb).unwrap();
            assert_eq!(get_creator_royalty(env, &a).unwrap().royalty_bps, 200);
            assert_eq!(get_creator_royalty(env, &b).unwrap().royalty_bps, 800);
        });
    }
}
