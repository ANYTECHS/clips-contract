//! Validation result structure (issue #1085).
//!
//! A standardized structure for representing validation success and failure
//! across every smart contract validator. All validators — whether they
//! implement [`crate::validators::Validator`] or are plain functions returning
//! `Result<(), Error>` — convert their outcome into a [`ValidationResult`] so
//! callers, pipelines, and guards share one consistent shape.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Support successful validation | [`ValidationResult::Valid`] + [`ValidationResult::valid`] |
//! | Support validation failures | [`ValidationResult::Invalid`] + [`ValidationResult::invalid`] |
//! | Integrate with centralized contract errors | Wraps [`crate::types::Error`]; `From` conversions both ways |
//! | Add unit tests | Tests module with comprehensive coverage |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::validators::ValidationResult;
//!
//! // Construct results directly.
//! let ok = ValidationResult::valid();
//! let err = ValidationResult::invalid(Error::InvalidAddress);
//!
//! // Convert from existing `Result<(), Error>` validators.
//! let result = ValidationResult::from_result(crate::royalty_validator::validate_royalty(&royalty));
//!
//! // Convert back when a `Result` is required (e.g. `?` propagation).
//! result.into_result()?;
//!
//! // Branch without unpacking.
//! if result.is_valid() {
//!     // proceed
//! }
//! ```

use crate::types::Error;

// ─── Core type ────────────────────────────────────────────────────────────────

/// Standardized outcome of a single validation check.
///
/// - [`ValidationResult::Valid`] — the input satisfied every constraint.
/// - [`ValidationResult::Invalid`] — the input failed validation; carries the
///   centralized contract [`Error`] describing the failure so callers can
///   propagate it without re-mapping error codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationResult {
    /// Validation succeeded.
    Valid,
    /// Validation failed with the enclosed centralized contract error.
    Invalid(Error),
}

// ─── Constructors ─────────────────────────────────────────────────────────────

impl ValidationResult {
    /// Build a successful validation result.
    pub const fn valid() -> Self {
        ValidationResult::Valid
    }

    /// Build a failed validation result carrying `error`.
    ///
    /// The error must be a centralized contract [`Error`] so that pipelines,
    /// guards, and entry points can propagate it directly.
    pub const fn invalid(error: Error) -> Self {
        ValidationResult::Invalid(error)
    }

    /// Build a result from a boolean condition.
    ///
    /// Returns [`ValidationResult::Valid`] when `condition` holds, otherwise
    /// [`ValidationResult::Invalid`] carrying `error`.
    pub const fn from_condition(condition: bool, error: Error) -> Self {
        if condition {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(error)
        }
    }

    /// Build a result from a conventional `Result<(), Error>` validator.
    ///
    /// This is the primary bridge between the existing function-style
    /// validators (e.g. `royalty_validator::validate_royalty`) and the
    /// standardized structure: `Ok(())` maps to [`ValidationResult::Valid`]
    /// and `Err(e)` maps to [`ValidationResult::Invalid(e)`].
    pub const fn from_result(result: Result<(), Error>) -> Self {
        match result {
            Ok(()) => ValidationResult::Valid,
            Err(e) => ValidationResult::Invalid(e),
        }
    }
}

// ─── Observers ────────────────────────────────────────────────────────────────

impl ValidationResult {
    /// Return `true` when validation succeeded.
    pub const fn is_valid(self) -> bool {
        match self {
            ValidationResult::Valid => true,
            ValidationResult::Invalid(_) => false,
        }
    }

    /// Return `true` when validation failed.
    pub const fn is_invalid(self) -> bool {
        match self {
            ValidationResult::Valid => false,
            ValidationResult::Invalid(_) => true,
        }
    }

    /// Return the enclosed error for failures, or `None` for success.
    pub const fn error(self) -> Option<Error> {
        match self {
            ValidationResult::Valid => None,
            ValidationResult::Invalid(e) => Some(e),
        }
    }

    /// Convert into a conventional `Result<(), Error>` for `?` propagation.
    ///
    /// [`ValidationResult::Valid`] maps to `Ok(())`; [`ValidationResult::Invalid(e)`]
    /// maps to `Err(e)`.
    pub const fn into_result(self) -> Result<(), Error> {
        match self {
            ValidationResult::Valid => Ok(()),
            ValidationResult::Invalid(e) => Err(e),
        }
    }

    /// Propagate the failure, if any, exactly like the `?` operator.
    ///
    /// Returns `Ok(())` on success; returns `Err(e)` on failure.
    pub const fn require(self) -> Result<(), Error> {
        self.into_result()
    }
}

// ─── Centralized-error integration ────────────────────────────────────────────

impl From<Result<(), Error>> for ValidationResult {
    /// Convert any `Result<(), Error>` validator outcome into the
    /// standardized structure without losing the centralized error code.
    fn from(result: Result<(), Error>) -> Self {
        ValidationResult::from_result(result)
    }
}

impl From<ValidationResult> for Result<(), Error> {
    /// Convert back into a `Result` so validation results compose with
    /// existing guard and entry-point code that uses `?`.
    fn from(result: ValidationResult) -> Self {
        result.into_result()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Successful validation ─────────────────────────────────────────────

    #[test]
    fn valid_constructor_reports_success() {
        let result = ValidationResult::valid();
        assert_eq!(result, ValidationResult::Valid);
        assert!(result.is_valid());
        assert!(!result.is_invalid());
        assert_eq!(result.error(), None);
        assert_eq!(result.into_result(), Ok(()));
    }

    #[test]
    fn ok_result_converts_to_valid() {
        let result = ValidationResult::from_result(Ok(()));
        assert_eq!(result, ValidationResult::Valid);
        assert!(result.is_valid());
    }

    #[test]
    fn from_ok_result_trait_conversion() {
        let result: ValidationResult = Ok(()).into();
        assert!(result.is_valid());
    }

    // ── Validation failures ───────────────────────────────────────────────

    #[test]
    fn invalid_constructor_reports_failure() {
        let result = ValidationResult::invalid(Error::InvalidAddress);
        assert_eq!(result, ValidationResult::Invalid(Error::InvalidAddress));
        assert!(!result.is_valid());
        assert!(result.is_invalid());
        assert_eq!(result.error(), Some(Error::InvalidAddress));
        assert_eq!(result.into_result(), Err(Error::InvalidAddress));
    }

    #[test]
    fn err_result_converts_to_invalid_preserving_error() {
        let result = ValidationResult::from_result(Err(Error::TokenNotFound));
        assert_eq!(result, ValidationResult::Invalid(Error::TokenNotFound));
        assert!(result.is_invalid());
    }

    #[test]
    fn from_err_result_trait_conversion_preserves_error() {
        let outcome: Result<(), Error> = Err(Error::Unauthorized);
        let result: ValidationResult = outcome.into();
        assert_eq!(result.error(), Some(Error::Unauthorized));
    }

    #[test]
    fn distinct_errors_are_preserved() {
        for error in [
            Error::InvalidAddress,
            Error::InvalidBasisPoints,
            Error::TokenNotFound,
            Error::Unauthorized,
            Error::UnauthorizedConfigurationUpdate,
            Error::RoyaltyFrozen,
            Error::ContractPaused,
        ] {
            let result = ValidationResult::invalid(error);
            assert_eq!(result.error(), Some(error));
            assert_eq!(result.into_result(), Err(error));
        }
    }

    // ── Centralized-error integration ─────────────────────────────────────

    #[test]
    fn validation_result_roundtrips_through_result() {
        let valid: ValidationResult = Ok(()).into();
        let back: Result<(), Error> = valid.into();
        assert_eq!(back, Ok(()));

        let invalid: ValidationResult = Err(Error::InvalidBasisPoints).into();
        let back: Result<(), Error> = invalid.into();
        assert_eq!(back, Err(Error::InvalidBasisPoints));
    }

    #[test]
    fn require_propagates_failure_like_try_operator() {
        assert!(ValidationResult::Valid.require().is_ok());
        assert_eq!(
            ValidationResult::Invalid(Error::ContractPaused).require(),
            Err(Error::ContractPaused)
        );
    }

    #[test]
    fn from_condition_maps_boolean_to_result() {
        assert!(ValidationResult::from_condition(true, Error::InvalidAddress).is_valid());
        assert_eq!(
            ValidationResult::from_condition(false, Error::InvalidAddress),
            ValidationResult::Invalid(Error::InvalidAddress)
        );
    }

    // ── Interop with existing validators ──────────────────────────────────

    #[test]
    fn wraps_existing_royalty_validator_outcome() {
        use crate::types::{Royalty, RoyaltyRecipient};
        use soroban_sdk::{testutils::Address as _, Address, Env};

        let env = Env::default();
        let recipient = Address::generate(&env);

        let good = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: recipient.clone(),
                    basis_points: 500,
                }
            ],
            asset_address: None,
        };
        assert!(ValidationResult::from_result(crate::royalty_validator::validate_royalty(
            &good
        ))
        .is_valid());

        let empty = Royalty {
            recipients: soroban_sdk::Vec::new(&env),
            asset_address: None,
        };
        assert_eq!(
            ValidationResult::from_result(crate::royalty_validator::validate_royalty(&empty)),
            ValidationResult::Invalid(Error::InvalidBasisPoints)
        );
    }
}
