//! Default royalty configuration (issue #781).
//!
//! Stores a contract-wide default royalty percentage (in basis points) that is
//! applied to newly minted NFTs when no per-token royalty is explicitly
//! provided.
//!
//! # Storage
//! Key: `DataKey::DefaultRoyaltyBps` (instance storage)
//!
//! # Limits
//! `0` – `10_000` bps inclusive (0 % – 100 %).
//! Typical values are in the range 100–1 000 bps (1 %–10 %).
//!
//! # Usage
//! ```ignore
//! // Set the contract-wide default royalty to 5%.
//! set_default_royalty_bps(&env, 500)?;
//!
//! // Read it back (falls back to DEFAULT_ROYALTY_BPS = 500 if never set).
//! let bps = get_default_royalty_bps(&env);
//! ```

use soroban_sdk::Env;

use crate::types::{DataKey, Error};

pub use crate::storage_constants::{DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS};

// ─── Public API ────────────────────────────────────────────────────────────────

/// Persist the contract-wide default royalty in basis points.
///
/// # Arguments
/// * `env` — Soroban execution environment.
/// * `bps` — Royalty in basis points. Must be in `0..=10_000` (0 %–100 %).
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when `bps > MAX_ROYALTY_BPS`.
///
/// # Storage
/// Written to `DataKey::DefaultRoyaltyBps` in instance storage.
pub fn set_default_royalty_bps(env: &Env, bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    env.storage()
        .instance()
        .set(&DataKey::DefaultRoyaltyBps, &bps);
    Ok(())
}

/// Return the stored default royalty in basis points.
///
/// Falls back to [`DEFAULT_ROYALTY_BPS`] (500 = 5 %) if the value has never
/// been explicitly set.
pub fn get_default_royalty_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::DefaultRoyaltyBps)
        .unwrap_or(DEFAULT_ROYALTY_BPS)
}

/// Return `true` if a default royalty has been explicitly stored.
///
/// `false` means [`get_default_royalty_bps`] will return the compile-time
/// fallback ([`DEFAULT_ROYALTY_BPS`]).
pub fn has_default_royalty_bps(env: &Env) -> bool {
    env.storage()
        .instance()
        .has(&DataKey::DefaultRoyaltyBps)
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── get_default_royalty_bps ───────────────────────────────────────────────

    #[test]
    fn returns_default_constant_before_any_set() {
        with_contract(|env| {
            assert_eq!(get_default_royalty_bps(env), DEFAULT_ROYALTY_BPS);
        });
    }

    // ── set_default_royalty_bps ───────────────────────────────────────────────

    #[test]
    fn set_and_get_round_trip() {
        with_contract(|env| {
            set_default_royalty_bps(env, 750).unwrap();
            assert_eq!(get_default_royalty_bps(env), 750);
        });
    }

    #[test]
    fn zero_bps_is_accepted() {
        with_contract(|env| {
            set_default_royalty_bps(env, 0).unwrap();
            assert_eq!(get_default_royalty_bps(env), 0);
        });
    }

    #[test]
    fn max_bps_is_accepted() {
        with_contract(|env| {
            set_default_royalty_bps(env, MAX_ROYALTY_BPS).unwrap();
            assert_eq!(get_default_royalty_bps(env), MAX_ROYALTY_BPS);
        });
    }

    #[test]
    fn above_max_bps_returns_invalid_basis_points() {
        with_contract(|env| {
            let result = set_default_royalty_bps(env, MAX_ROYALTY_BPS + 1);
            assert_eq!(result, Err(Error::InvalidBasisPoints));
        });
    }

    #[test]
    fn large_value_returns_invalid_basis_points() {
        with_contract(|env| {
            assert_eq!(
                set_default_royalty_bps(env, 50_000),
                Err(Error::InvalidBasisPoints),
            );
        });
    }

    #[test]
    fn u32_max_returns_invalid_basis_points() {
        with_contract(|env| {
            assert_eq!(
                set_default_royalty_bps(env, u32::MAX),
                Err(Error::InvalidBasisPoints),
            );
        });
    }

    #[test]
    fn overwrite_stores_latest_value() {
        with_contract(|env| {
            set_default_royalty_bps(env, 100).unwrap();
            set_default_royalty_bps(env, 9_000).unwrap();
            assert_eq!(get_default_royalty_bps(env), 9_000);
        });
    }

    // ── has_default_royalty_bps ───────────────────────────────────────────────

    #[test]
    fn has_returns_false_before_set() {
        with_contract(|env| {
            assert!(!has_default_royalty_bps(env));
        });
    }

    #[test]
    fn has_returns_true_after_set() {
        with_contract(|env| {
            set_default_royalty_bps(env, 500).unwrap();
            assert!(has_default_royalty_bps(env));
        });
    }
}
