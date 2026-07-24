//! Per-token royalty percentage storage (issue #673).
//!
//! Persists the royalty percentage — expressed in basis points (bps), matching
//! the rest of the contract — associated with every minted NFT, validates it
//! against the allowed maximum, and reads it back.
//!
//! # Storage
//! Key: `DataKey::RoyaltyPercentage(token_id)` (persistent storage)
//!
//! # Limits
//! `0` – `10_000` bps inclusive (0 % – 100 %).

use soroban_sdk::Env;

use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::{DataKey, Error, TokenId};

/// Persist the royalty percentage (in basis points) for `token_id`.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when `bps > MAX_ROYALTY_BPS`.
pub fn set_royalty_percentage(env: &Env, token_id: TokenId, bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyPercentage(token_id), &bps);
    Ok(())
}

/// Return the royalty percentage (in basis points) for `token_id`.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if no percentage has been recorded.
pub fn get_royalty_percentage(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::RoyaltyPercentage(token_id))
        .ok_or(Error::TokenNotFound)
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
    fn stores_and_retrieves_percentage() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 500).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 500);
        });
    }

    #[test]
    fn accepts_boundary_values() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 0).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 0);

            set_royalty_percentage(env, 2, MAX_ROYALTY_BPS).unwrap();
            assert_eq!(get_royalty_percentage(env, 2).unwrap(), MAX_ROYALTY_BPS);
        });
    }

    #[test]
    fn rejects_percentage_above_limit() {
        with_contract(|env| {
            assert_eq!(
                set_royalty_percentage(env, 1, MAX_ROYALTY_BPS + 1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn overwrites_existing_percentage() {
        with_contract(|env| {
            set_royalty_percentage(env, 1, 250).unwrap();
            set_royalty_percentage(env, 1, 750).unwrap();
            assert_eq!(get_royalty_percentage(env, 1).unwrap(), 750);
        });
    }

    #[test]
    fn missing_percentage_returns_not_found() {
        with_contract(|env| {
            assert_eq!(get_royalty_percentage(env, 99), Err(Error::TokenNotFound));
        });
    }
}
