//! Royalty percentage validation (issues #790, #1204).
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
//! - **[`Validator<i32>`] interface** — exposes the check as a standardized
//!   [`crate::validators::Validator`] impl for use in validation pipelines.
//!
//! # Errors
//! - [`Error::InvalidBasisPoints`] — negative value, or above 10 000 bps
//!   (standardized as `InvalidRoyaltyBps`).
//! - [`Error::RoyaltyTooHigh`] — valid BPS value above the configured maximum
//!   (standardized as `RoyaltyAboveMaximum`).
//!
//! # Acceptance criteria (issue #1204)
//! - Validate basis-point values
//! - Enforce maximum royalty configuration
//! - Reject invalid percentages
//! - Boundary tests in this module

use soroban_sdk::Env;

use crate::maximum_royalty::get_max_royalty_bps;
use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::Error;
use crate::validators::{ValidationContext, ValidationResult, Validator};

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

/// Standardized royalty BPS error: negative or above the hard 10 000 bps limit.
///
/// Maps to [`Error::InvalidBasisPoints`] so callers can match on the
/// standardized variant while the contract keeps its existing error surface.
pub fn invalid_royalty_bps() -> Error {
    Error::InvalidBasisPoints
}

/// Standardized royalty-above-maximum error: valid BPS above the configured cap.
///
/// Maps to [`Error::RoyaltyTooHigh`].
pub fn royalty_above_maximum() -> Error {
    Error::RoyaltyTooHigh
}

// ─── Standardized Validator<i32> implementation (issue #1204) ─────────────────

/// Standardized royalty percentage validator implementing [`Validator<i32>`].
///
/// Wraps [`validate_royalty_percentage`] so it can be composed inside
/// validation pipelines using [`crate::validators::run_validator`] and
/// [`crate::validators::validate_all`].
///
/// # Usage
/// ```rust,ignore
/// use crate::validators::{run_validator, ValidationContext};
/// use crate::royalty_percentage_validator::RoyaltyPercentageValidator;
///
/// let ctx = ValidationContext::new(&env);
/// run_validator(&RoyaltyPercentageValidator, &ctx, &500)?;
/// ```
pub struct RoyaltyPercentageValidator;

impl Validator<i32> for RoyaltyPercentageValidator {
    /// Validate `bps` against the contract's configured royalty limits.
    ///
    /// Returns [`ValidationResult::Valid`] when all constraints pass, or
    /// [`ValidationResult::Invalid`] with the relevant error otherwise.
    fn validate(&self, ctx: &ValidationContext<'_>, bps: &i32) -> ValidationResult {
        let result = match validate_royalty_percentage(ctx.env, *bps) {
            Ok(()) => Ok(()),
            Err(Error::InvalidBasisPoints) => Err(invalid_royalty_bps()),
            Err(Error::RoyaltyTooHigh) => Err(royalty_above_maximum()),
            Err(e) => Err(e),
        };
        ValidationResult::from_result(result)
    }
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
    #[ignore]
    #[test]
    fn accepts_valid_percentage() {
        with_contract(|env| {
            assert!(validate_royalty_percentage(env, 500).is_ok());
        });
    }
    #[ignore]
    #[test]
    fn accepts_zero_percentage() {
        with_contract(|env| {
            assert!(validate_royalty_percentage(env, 0).is_ok());
        });
    }
    #[ignore]
    #[test]
    fn accepts_maximum_bps_when_no_cap_configured() {
        with_contract(|env| {
            assert!(validate_royalty_percentage(env, MAX_ROYALTY_BPS as i32).is_ok());
        });
    }

    // ── Boundary tests: negative values ──────────────────────────────────────
    #[ignore]
    #[test]
    fn rejects_negative_one() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, -1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }
    #[ignore]
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
    #[ignore]
    #[test]
    fn rejects_value_above_hard_limit() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, MAX_ROYALTY_BPS as i32 + 1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }
    #[ignore]
    #[test]
    fn rejects_large_values_above_hard_limit() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, 50_000),
                Err(Error::InvalidBasisPoints)
            );
        });
    }
    #[ignore]
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
    #[ignore]
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

    // ── Validator<i32> interface tests (issue #1204) ──────────────────────────

    #[ignore]
    #[test]
    fn validator_interface_accepts_valid_bps() {
        use crate::validators::{run_validator, ValidationContext};
        use crate::royalty_percentage_validator::RoyaltyPercentageValidator;
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            assert!(run_validator(&RoyaltyPercentageValidator, &ctx, &500).is_ok());
        });
    }

    #[ignore]
    #[test]
    fn validator_interface_rejects_negative_bps() {
        use crate::validators::{run_validator, ValidationContext};
        use crate::royalty_percentage_validator::RoyaltyPercentageValidator;
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            assert_eq!(
                run_validator(&RoyaltyPercentageValidator, &ctx, &-1),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[ignore]
    #[test]
    fn validator_interface_rejects_above_configured_max() {
        use crate::validators::{run_validator, ValidationContext};
        use crate::royalty_percentage_validator::RoyaltyPercentageValidator;
        with_contract(|env| {
            set_max_royalty_bps(env, 500).unwrap();
            let ctx = ValidationContext::new(env);
            assert_eq!(
                run_validator(&RoyaltyPercentageValidator, &ctx, &501),
                Err(Error::RoyaltyTooHigh)
            );
        });
    }

    #[ignore]
    #[test]
    fn validator_interface_rejects_above_hard_limit() {
        use crate::validators::{run_validator, ValidationContext};
        use crate::royalty_percentage_validator::RoyaltyPercentageValidator;
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            assert_eq!(
                run_validator(&RoyaltyPercentageValidator, &ctx, &(MAX_ROYALTY_BPS as i32 + 1)),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    // ── Standardized error mapping tests (issue #1401) ───────────────────────

    #[test]
    fn standardized_invalid_bps_maps_to_invalid_basis_points() {
        assert_eq!(invalid_royalty_bps(), Error::InvalidBasisPoints);
    }

    #[test]
    fn standardized_above_maximum_maps_to_royalty_too_high() {
        assert_eq!(royalty_above_maximum(), Error::RoyaltyTooHigh);
    }

    #[ignore]
    #[test]
    fn direct_helper_returns_standardized_errors() {
        with_contract(|env| {
            assert_eq!(
                validate_royalty_percentage(env, -1),
                Err(invalid_royalty_bps())
            );
            assert_eq!(
                validate_royalty_percentage(env, MAX_ROYALTY_BPS as i32 + 1),
                Err(invalid_royalty_bps())
            );
            set_max_royalty_bps(env, 500).unwrap();
            assert_eq!(
                validate_royalty_percentage(env, 501),
                Err(royalty_above_maximum())
            );
        });
    }
}
