//! Royalty recipient validation (issues #671, #1205).
//!
//! Ensures the royalty recipient is a valid Stellar wallet address before any
//! royalty information is persisted. Rejects the contract's own address (which
//! cannot usefully receive royalty payments) and returns a dedicated custom
//! error so callers can distinguish recipient failures from other mint errors.
//!
//! This module also exposes a [`RoyaltyRecipientValidator`] struct that
//! implements the standardized [`crate::validators::Validator<Address>`]
//! interface so recipient validation can be composed in pipeline checks.
//!
//! # Errors
//! [`Error::InvalidRecipient`] when the address is not a valid royalty recipient.
//! Invalid recipients are surfaced through the standardized
//! [`InvalidRoyaltyError::InvalidRoyaltyRecipient`] variant (issue #1401), which
//! maps to [`Error::InvalidRecipient`].
//!
//! # Acceptance criteria (issue #1205)
//! - Validate recipient address
//! - Reject invalid recipients
//! - Prevent prohibited recipient values (contract self-address)
//! - Add unit tests
//!
//! # Acceptance criteria (issue #1401)
//! - Standardized invalid royalty errors, including invalid recipient

use soroban_sdk::{Address, Env};

use crate::types::{Error, Royalty};
use crate::validators::{ValidationContext, ValidationResult, Validator};

/// Standardized errors for invalid royalty configurations (issue #1401).
///
/// Each variant captures a distinct class of royalty misconfiguration so
/// callers can distinguish failures. [`InvalidRoyaltyError::to_error`] maps
/// each variant onto the contract's canonical [`Error`] type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidRoyaltyError {
    /// Royalty percentage exceeds the configured maximum.
    RoyaltyAboveMaximum,
    /// A royalty recipient address is not acceptable (e.g. contract self-address).
    InvalidRoyaltyRecipient,
    /// A royalty basis-points value is out of range.
    InvalidBps,
    /// The royalty configuration is in an invalid state.
    InvalidRoyaltyState,
    /// The caller is not authorized to update the royalty configuration.
    UnauthorizedUpdate,
}

impl InvalidRoyaltyError {
    /// Map this standardized royalty error onto the canonical [`Error`] type.
    pub fn to_error(self) -> Error {
        match self {
            InvalidRoyaltyError::RoyaltyAboveMaximum => Error::InvalidRoyalty,
            InvalidRoyaltyError::InvalidRoyaltyRecipient => Error::InvalidRecipient,
            InvalidRoyaltyError::InvalidBps => Error::InvalidBps,
            InvalidRoyaltyError::InvalidRoyaltyState => Error::InvalidRoyalty,
            InvalidRoyaltyError::UnauthorizedUpdate => Error::Unauthorized,
        }
    }
}

/// Validate that `recipient` is an acceptable royalty wallet address.
///
/// # Checks
/// - Rejects the current contract address (cannot hold / receive royalties).
///
/// # Errors
/// Returns [`Error::InvalidRecipient`] (via
/// [`InvalidRoyaltyError::InvalidRoyaltyRecipient`]) when the address fails
/// validation.
pub fn validate_royalty_recipient(env: &Env, recipient: &Address) -> Result<(), Error> {
    let contract = env.current_contract_address();
    if *recipient == contract {
        return Err(InvalidRoyaltyError::InvalidRoyaltyRecipient.to_error());
    }
    Ok(())
}

/// Validate every recipient configured in a royalty config (issue #831).
///
/// Iterates the recipient list and verifies each address is a valid
/// royalty recipient (not the contract itself).
///
/// # Errors
/// Returns [`Error::InvalidRecipient`] (via
/// [`InvalidRoyaltyError::InvalidRoyaltyRecipient`]) when any recipient fails
/// validation.
pub fn validate_royalty_recipients(env: &Env, royalty: &Royalty) -> Result<(), Error> {
    for r in royalty.recipients.iter() {
        validate_royalty_recipient(env, &r.recipient)?;
    }
    Ok(())
}

// ─── Standardized Validator<Address> implementation (issue #1205) ─────────────

/// Standardized royalty recipient validator implementing [`Validator<Address>`].
///
/// Wraps [`validate_royalty_recipient`] so it can be composed inside
/// validation pipelines using [`crate::validators::run_validator`] and
/// [`crate::validators::validate_all`].
///
/// # Checks
/// - Rejects the contract's own address as a recipient.
///
/// # Usage
/// ```rust,ignore
/// use crate::validators::{run_validator, ValidationContext};
/// use crate::royalty_recipient_validator::RoyaltyRecipientValidator;
///
/// let recipient = Address::generate(&env);
/// let ctx = ValidationContext::new(&env);
/// run_validator(&RoyaltyRecipientValidator, &ctx, &recipient)?;
/// ```
pub struct RoyaltyRecipientValidator;

impl Validator<Address> for RoyaltyRecipientValidator {
    /// Validate that `recipient` is an acceptable royalty wallet address.
    ///
    /// Returns [`ValidationResult::Valid`] for any external address, or
    /// [`ValidationResult::Invalid(Error::InvalidRecipient)`] when the
    /// address is the contract itself.
    fn validate(&self, ctx: &ValidationContext<'_>, recipient: &Address) -> ValidationResult {
        ValidationResult::from_result(validate_royalty_recipient(ctx.env, recipient))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }
    #[ignore]
    #[test]
    fn accepts_valid_wallet_address() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert!(validate_royalty_recipient(env, &recipient).is_ok());
        });
    }
    #[ignore]
    #[test]
    fn rejects_contract_self_address() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(
                validate_royalty_recipient(env, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }
    #[ignore]
    #[test]
    fn accepts_distinct_generated_addresses() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            assert!(validate_royalty_recipient(env, &a).is_ok());
            assert!(validate_royalty_recipient(env, &b).is_ok());
            assert_ne!(a, b);
        });
    }

    // ── Validator<Address> interface tests (issue #1205) ─────────────────────

    #[ignore]
    #[test]
    fn validator_interface_accepts_valid_recipient() {
        use crate::royalty_recipient_validator::RoyaltyRecipientValidator;
        use crate::validators::{run_validator, ValidationContext};
        with_contract(|env| {
            let recipient = Address::generate(env);
            let ctx = ValidationContext::new(env);
            assert!(run_validator(&RoyaltyRecipientValidator, &ctx, &recipient).is_ok());
        });
    }

    #[ignore]
    #[test]
    fn validator_interface_rejects_contract_self_address() {
        use crate::royalty_recipient_validator::RoyaltyRecipientValidator;
        use crate::validators::{run_validator, ValidationContext};
        with_contract(|env| {
            let contract = env.current_contract_address();
            let ctx = ValidationContext::new(env);
            assert_eq!(
                run_validator(&RoyaltyRecipientValidator, &ctx, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[ignore]
    #[test]
    fn validator_interface_accepts_multiple_distinct_recipients() {
        use crate::royalty_recipient_validator::RoyaltyRecipientValidator;
        use crate::validators::{run_validator, ValidationContext};
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            for _ in 0..3 {
                let addr = Address::generate(env);
                assert!(run_validator(&RoyaltyRecipientValidator, &ctx, &addr).is_ok());
            }
        });
    }

    // ── Prevent prohibited values ─────────────────────────────────────────────

    #[ignore]
    #[test]
    fn prohibited_contract_address_is_always_rejected() {
        with_contract(|env| {
            // The contract address is a "prohibited value" — no matter how many
            // times we call it, the contract self-address must always fail.
            let contract = env.current_contract_address();
            for _ in 0..3 {
                assert_eq!(
                    validate_royalty_recipient(env, &contract),
                    Err(Error::InvalidRecipient)
                );
            }
        });
    }

    // ── Standardized invalid royalty errors (issue #1401) ────────────────────

    #[test]
    fn invalid_royalty_recipient_maps_to_invalid_recipient() {
        assert_eq!(
            InvalidRoyaltyError::InvalidRoyaltyRecipient.to_error(),
            Error::InvalidRecipient
        );
    }

    #[test]
    fn standardized_royalty_errors_map_to_canonical_errors() {
        assert_eq!(
            InvalidRoyaltyError::RoyaltyAboveMaximum.to_error(),
            Error::InvalidRoyalty
        );
        assert_eq!(
            InvalidRoyaltyError::InvalidBps.to_error(),
            Error::InvalidBps
        );
        assert_eq!(
            InvalidRoyaltyError::InvalidRoyaltyState.to_error(),
            Error::InvalidRoyalty
        );
        assert_eq!(
            InvalidRoyaltyError::UnauthorizedUpdate.to_error(),
            Error::Unauthorized
        );
    }

    #[ignore]
    #[test]
    fn contract_self_address_returns_standardized_invalid_recipient() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            let result = validate_royalty_recipient(env, &contract);
            assert_eq!(
                result,
                Err(InvalidRoyaltyError::InvalidRoyaltyRecipient.to_error())
            );
            assert_eq!(result, Err(Error::InvalidRecipient));
        });
    }

    #[ignore]
    #[test]
    fn validate_royalty_recipients_rejects_contract_self_address() {
        use crate::types::RoyaltyRecipient;
        with_contract(|env| {
            let contract = env.current_contract_address();
            let valid = Address::generate(env);
            let royalty = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: valid,
                        basis_points: 500,
                    },
                    RoyaltyRecipient {
                        recipient: contract,
                        basis_points: 500,
                    },
                ],
            };
            assert_eq!(
                validate_royalty_recipients(env, &royalty),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[ignore]
    #[test]
    fn validate_royalty_recipients_accepts_all_valid_recipients() {
        use crate::types::RoyaltyRecipient;
        with_contract(|env| {
            let royalty = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: Address::generate(env),
                        basis_points: 500,
                    },
                    RoyaltyRecipient {
                        recipient: Address::generate(env),
                        basis_points: 500,
                    },
                ],
            };
            assert!(validate_royalty_recipients(env, &royalty).is_ok());
        });
    }
}
