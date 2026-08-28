//! Royalty percentage validation (issue #790).
//!
//! Ensures any royalty percentage submitted to the contract falls within the
//! configured contract limits before it is accepted:
//!
//! - **Reject negative values** — any value below zero is rejected outright.
//! - **Validate BPS** — the percentage must be a valid basis-points figure
//!   (`0..=10_000`, i.e. 0 %–100 % of the sale price).
//! - **Reject values above maximum** — the percentage must also respect the
//!   configurable contract-wide maximum royalty (issue #782), which defaults
//!   to [`MAX_ROYALTY_BPS`] when no explicit cap has been configured.
//!
//! # Errors
//! - [`Error::InvalidBasisPoints`] — negative value, or above 10 000 bps.
//! - [`Error::RoyaltyTooHigh`] — valid BPS value above the configured maximum.
//!
//! # Acceptance criteria
//! - Reject negative values
//! - Reject values above maximum
//! - Validate BPS
//! - Boundary tests in this module

use soroban_sdk::Env;

use crate::maximum_royalty::get_max_royalty_bps;
use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::Error;

/// Validate a royalty percentage (expressed in basis points) against the
/// contract's configured limits.
///
/// `bps` is accepted as a signed integer so that negative values can be
/// detected and rejected explicitly — the strong-typed `u32` fields used
/// elsewhere in the contract cannot represent a negative percentage.
pub fn validate_royalty_percentage(env: &Env, bps: i32) -> Result<(), Error> {
    if bps < 0 {
        return Err(Error::InvalidBasisPoints);
    }
    let bps = u32::try_from(bps).map_err(|_| Error::InvalidBasisPoints)?;
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if bps > get_max_royalty_bps(env) {
        return Err(Error::RoyaltyTooHigh);
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maximum_royalty::set_max_royalty_bps;
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

    // ── Valid values ─────────────────────────────────────────────────────────

    #[test]
    fn accepts_valid_percentage() {
        with_contract(|env| {
            assert!(validate_royalty_percentage(env, 500).is_ok());
        });
    }

    #[test]
    fn accepts_zero_percentage() {
        with_contract(|env| {
            assert!(validate_royalty_percentage(env, 0).is_ok());
        });
    }

    #[test]
    fn accepts_maximum_bps_when_no_cap_configured() {
        with_contract(|env| {
            assert!(validate_royalty_percentage(env, MAX_ROYALTY_BPS as i32).is_ok());
        });
    }

    // ── Boundary tests: negative values ──────────────────────────────────────

    #[test]
    fn rejects_negative_one() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, -1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn rejects_large_negative_values() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, -10_000),
                Err(Error::InvalidBasisPoints)
            );
            assert_eq!(
                validate_royalty_percentage(env, i32::MIN),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    // ── Boundary tests: values above maximum ─────────────────────────────────

    #[test]
    fn rejects_value_above_hard_limit() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, MAX_ROYALTY_BPS as i32 + 1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn rejects_large_values_above_hard_limit() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, 50_000),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn respects_configured_maximum() {
        with_contract(|env| {
            set_max_royalty_bps(env, 1_000).unwrap();

            assert!(validate_royalty_percentage(env, 1_000).is_ok());
            assert_eq!(
                validate_royalty_percentage(env, 1_001),
                Err(Error::RoyaltyTooHigh)
            );
            assert_eq!(
                validate_royalty_percentage(env, 5_000),
                Err(Error::RoyaltyTooHigh)
            );
        });
    }

    #[test]
    fn zero_maximum_allows_only_zero() {
        with_contract(|env| {
            set_max_royalty_bps(env, 0).unwrap();

            assert!(validate_royalty_percentage(env, 0).is_ok());
            assert_eq!(
                validate_royalty_percentage(env, 1),
                Err(Error::RoyaltyTooHigh)
            );
        });
    }
}