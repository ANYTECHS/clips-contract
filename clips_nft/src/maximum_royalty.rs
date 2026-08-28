//! Configurable maximum royalty limit (issue #782).
//!
//! Provides a contract-wide, operator-configurable ceiling on the royalty
//! percentage (in basis points) that may be assigned to any token.  This
//! prevents excessively large royalty percentages from being configured by
//! restricting every royalty write to a value at or below the stored maximum.
//!
//! # Storage
//! Key: `DataKey::MaximumRoyaltyBps` (instance storage)
//!
//! # Fallback
//! When no explicit maximum has been configured, the hard upper bound
//! [`MAX_ROYALTY_BPS`] (10 000 bps = 100 %) is used so existing behaviour is
//! preserved until an operator opts in to a stricter cap.
//!
//! # Acceptance criteria
//! - **Store maximum royalty** — [`set_max_royalty_bps`]
//! - **Validate updates** — values above [`MAX_ROYALTY_BPS`] are rejected
//! - **Prevent royalty above maximum** — [`validate_royalty_within_max`]

use soroban_sdk::Env;

use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::{DataKey, Error};

// ─── Public API ───────────────────────────────────────────────────────────────

/// Persist the configurable maximum royalty limit in basis points.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when `bps > MAX_ROYALTY_BPS` (10 000).
///
/// # Storage
/// Written to `DataKey::MaximumRoyaltyBps` in instance storage.
pub fn set_max_royalty_bps(env: &Env, bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    env.storage()
        .instance()
        .set(&DataKey::MaximumRoyaltyBps, &bps);
    Ok(())
}

/// Return the configurable maximum royalty in basis points.
///
/// Falls back to [`MAX_ROYALTY_BPS`] (10 000 = 100 %) if never explicitly set.
pub fn get_max_royalty_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MaximumRoyaltyBps)
        .unwrap_or(MAX_ROYALTY_BPS)
}

/// Return `true` if an explicit maximum royalty has been configured.
pub fn has_max_royalty_bps(env: &Env) -> bool {
    env.storage()
        .instance()
        .has(&DataKey::MaximumRoyaltyBps)
}

/// Return `true` if `bps` is at or below the configured maximum royalty.
pub fn allowed_royalty_bps(env: &Env, bps: u32) -> bool {
    bps <= get_max_royalty_bps(env)
}

/// Verify that `bps` respects the configured maximum royalty.
///
/// # Errors
/// Returns [`Error::RoyaltyTooHigh`] when `bps > get_max_royalty_bps(env)`.
pub fn validate_royalty_within_max(env: &Env, bps: u32) -> Result<(), Error> {
    if bps > get_max_royalty_bps(env) {
        return Err(Error::RoyaltyTooHigh);
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

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

    // ── get_max_royalty_bps ───────────────────────────────────────────────────

    #[test]
    fn returns_max_constant_before_any_set() {
        with_contract(|env| {
            assert_eq!(get_max_royalty_bps(env), MAX_ROYALTY_BPS);
            assert!(!has_max_royalty_bps(env));
        });
    }

    // ── set_max_royalty_bps ───────────────────────────────────────────────────

    #[test]
    fn set_and_get_round_trip() {
        with_contract(|env| {
            set_max_royalty_bps(env, 1_000).unwrap();
            assert_eq!(get_max_royalty_bps(env), 1_000);
            assert!(has_max_royalty_bps(env));
        });
    }

    #[test]
    fn zero_is_accepted() {
        with_contract(|env| {
            set_max_royalty_bps(env, 0).unwrap();
            assert_eq!(get_max_royalty_bps(env), 0);
        });
    }

    #[test]
    fn max_constant_is_accepted() {
        with_contract(|env| {
            set_max_royalty_bps(env, MAX_ROYALTY_BPS).unwrap();
            assert_eq!(get_max_royalty_bps(env), MAX_ROYALTY_BPS);
        });
    }

    #[test]
    fn above_max_constant_rejected() {
        with_contract(|env| {
            assert_eq!(
                set_max_royalty_bps(env, MAX_ROYALTY_BPS + 1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn large_value_rejected() {
        with_contract(|env| {
            assert_eq!(set_max_royalty_bps(env, 50_000), Err(Error::InvalidBasisPoints));
        });
    }

    #[test]
    fn overwrite_stores_latest_value() {
        with_contract(|env| {
            set_max_royalty_bps(env, 100).unwrap();
            set_max_royalty_bps(env, 5_000).unwrap();
            assert_eq!(get_max_royalty_bps(env), 5_000);
        });
    }

    // ── allowed_royalty_bps / validate_royalty_within_max ─────────────────────

    #[test]
    fn default_max_allows_up_to_ten_thousand() {
        with_contract(|env| {
            assert!(allowed_royalty_bps(env, MAX_ROYALTY_BPS));
            assert!(validate_royalty_within_max(env, MAX_ROYALTY_BPS).is_ok());
            assert!(!allowed_royalty_bps(env, MAX_ROYALTY_BPS + 1));
            assert_eq!(
                validate_royalty_within_max(env, MAX_ROYALTY_BPS + 1),
                Err(Error::RoyaltyTooHigh)
            );
        });
    }

    #[test]
    fn stricter_max_rejects_previously_valid_values() {
        with_contract(|env| {
            set_max_royalty_bps(env, 1_000).unwrap();
            assert!(allowed_royalty_bps(env, 1_000));
            assert!(validate_royalty_within_max(env, 1_000).is_ok());

            assert!(!allowed_royalty_bps(env, 1_001));
            assert_eq!(
                validate_royalty_within_max(env, 1_001),
                Err(Error::RoyaltyTooHigh)
            );
        });
    }

    #[test]
    fn zero_max_allows_only_zero() {
        with_contract(|env| {
            set_max_royalty_bps(env, 0).unwrap();
            assert!(allowed_royalty_bps(env, 0));
            assert!(!allowed_royalty_bps(env, 1));
        });
    }
}
