//! Total NFT supply counter.
//!
//! Increments and persists the global supply after every successful mint,
//! using checked arithmetic to prevent overflow.
//!
//! # Storage
//! Key: [`DataKey::TotalSupply`] (instance storage)

use soroban_sdk::Env;

use crate::storage_constants::DEFAULT_TOTAL_SUPPLY;
use crate::types::{DataKey, Error};

/// Return the current total NFT supply (defaults to `0`).
pub fn get_total_supply(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(DEFAULT_TOTAL_SUPPLY)
}

/// Increment total supply by one and persist it.
///
/// # Errors
/// Returns [`Error::SupplyOverflow`] when the counter would exceed `u32::MAX`.
pub fn increment_total_supply(env: &Env) -> Result<u32, Error> {
    let current = get_total_supply(env);
    let next = current
        .checked_add(1)
        .ok_or(Error::SupplyOverflow)?;
    env.storage()
        .instance()
        .set(&DataKey::TotalSupply, &next);
    Ok(next)
}

/// Persist an explicit total-supply value (used by migrations / tests).
pub fn set_total_supply(env: &Env, supply: u32) {
    env.storage()
        .instance()
        .set(&DataKey::TotalSupply, &supply);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::Env;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn starts_at_zero() {
        with_contract(|env| {
            assert_eq!(get_total_supply(env), 0);
        });
    }

    #[test]
    fn increments_and_persists() {
        with_contract(|env| {
            assert_eq!(increment_total_supply(env).unwrap(), 1);
            assert_eq!(get_total_supply(env), 1);
            assert_eq!(increment_total_supply(env).unwrap(), 2);
            assert_eq!(get_total_supply(env), 2);
        });
    }

    #[test]
    fn prevents_overflow() {
        with_contract(|env| {
            set_total_supply(env, u32::MAX);
            assert_eq!(increment_total_supply(env), Err(Error::SupplyOverflow));
            assert_eq!(get_total_supply(env), u32::MAX);
        });
    }
}
