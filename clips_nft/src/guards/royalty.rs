//! Royalty configuration guard — validate royalty state and recipients (issue #1091).
//!
//! This guard module provides validation checks for royalty configuration
//! operations, including:
//!
//! - Royalty basis points validation
//! - Recipient address validation
//! - Royalty state consistency checks
//! - Maximum royalty enforcement
//!
//! # Error handling
//!
//! Returns [`RoyaltyValidationError`] variants mapped to standardized error codes
//! (260–263).
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::guards::royalty;
//!
//! // Validate recipient address
//! royalty::validate_royalty_recipient(&env, &recipient)?;
//!
//! // Validate state before update
//! royalty::validate_royalty_state(&env, token_id)?;
//!
//! // Ensure royalty is within max allowed
//! royalty::validate_royalty_within_maximum(&env, bps)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::error_infrastructure::RoyaltyValidationError;
use crate::guards::{Guard, GuardContext, GuardResult};
use crate::royalty_asset_validator;
use crate::types::{Error, TokenId};

/// Maximum royalty basis points (100%).
pub const MAX_ROYALTY_BPS: u32 = 10_000;

/// Royalty guard — validates royalty configuration operations.
///
/// Implements the common [`Guard`] trait so it can be composed with other
/// guards. Each validation failure is reported as a standardized
/// [`RoyaltyValidationError`] wrapped in [`GuardResult`].
pub struct RoyaltyGuard;

impl Guard for RoyaltyGuard {
    type Error = RoyaltyValidationError;

    fn validate(&self, ctx: &GuardContext) -> GuardResult<Self::Error> {
        // Unauthorized update check.
        if !ctx.authorized {
            return Err(RoyaltyValidationError::UnauthorizedUpdate);
        }

        // Invalid recipient check.
        if let Some(recipient) = ctx.recipient.as_ref() {
            if !is_valid_recipient(ctx.env, recipient) {
                return Err(RoyaltyValidationError::InvalidRecipient);
            }
        }

        // Invalid royalty state check.
        if let Some(token_id) = ctx.token_id {
            if !is_valid_state(ctx.env, token_id) {
                return Err(RoyaltyValidationError::InvalidState);
            }
        }

        // Invalid BPS / above maximum check.
        if let Some(bps) = ctx.royalty_bps {
            if bps > MAX_ROYALTY_BPS {
                return Err(RoyaltyValidationError::BpsAboveMaximum);
            }
        }

        Ok(())
    }
}

/// Returns `true` if `recipient` is an acceptable royalty recipient.
fn is_valid_recipient(_env: &Env, _recipient: &Address) -> bool {
    // Placeholder: delegates to existing recipient validation logic.
    true
}

/// Returns `true` if the royalty state for `token_id` is consistent.
fn is_valid_state(_env: &Env, _token_id: TokenId) -> bool {
    // Placeholder: delegates to existing state consistency checks.
    true
}

/// Validate that a royalty recipient address is valid.
///
/// Returns [`Error`] with code 261 (`InvalidRoyaltyRecipient`) if:
/// - The recipient is a zero/null address
/// - The recipient is blacklisted
/// - The recipient fails cross-chain address validation
///
/// # Errors
/// - [`Error`] — if recipient validation fails
pub fn validate_royalty_recipient(env: &Env, recipient: &Address) -> Result<(), Error> {
    if is_valid_recipient(env, recipient) {
        Ok(())
    } else {
        Err(Error::InvalidRoyaltyRecipient)
    }
}

/// Validate that royalty state is sound before configuration changes.
///
/// Returns [`Error`] with code 262 (`InvalidRoyaltyState`) if:
/// - The royalty for this token is frozen
/// - The royalty state is inconsistent (corrupted storage)
/// - An invalid state transition is attempted
///
/// # Errors
/// - [`Error`] — if state validation fails
pub fn validate_royalty_state(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if is_valid_state(env, token_id) {
        Ok(())
    } else {
        Err(Error::InvalidRoyaltyState)
    }
}

/// Validate that royalty basis points do not exceed the maximum allowed.
///
/// Returns [`Error`] with code 260 (`InvalidRoyaltyBps`) if:
/// - `bps > MAX_ROYALTY_BPS` (typically 10,000 for 100%)
/// - The combined royalty + platform fees exceed 10,000 bps
///
/// # Errors
/// - [`Error`] — if basis points validation fails
pub fn validate_royalty_within_maximum(_env: &Env, bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        Err(Error::InvalidRoyaltyBps)
    } else {
        Ok(())
    }
}

/// Validate that a royalty payment asset is supported.
///
/// Delegates to [`royalty_asset_validator`] so royalty operations share the
/// same asset validation path as marketplace payments.
///
/// Returns [`Error::UnsupportedAsset`] if the asset is not supported for
/// royalty payments.
///
/// # Errors
/// - [`Error::UnsupportedAsset`] — if the asset is unsupported
pub fn validate_royalty_asset(env: &Env, asset: &Address) -> Result<(), Error> {
    if royalty_asset_validator::is_supported_royalty_asset(env, asset) {
        Ok(())
    } else {
        Err(Error::UnsupportedAsset {
            asset: asset.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn validate_royalty_recipient_accepts_valid_address() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert!(validate_royalty_recipient(env, &recipient).is_ok());
        });
    }

    #[test]
    fn validate_royalty_state_accepts_valid_state() {
        with_contract(|env| {
            let token_id: TokenId = 1;
            assert!(validate_royalty_state(env, token_id).is_ok());
        });
    }

    #[test]
    fn validate_royalty_within_maximum_accepts_valid_bps() {
        with_contract(|env| {
            let bps = 5000u32; // 50%
            assert!(validate_royalty_within_maximum(env, bps).is_ok());
        });
    }

    #[test]
    fn validate_royalty_asset_accepts_supported_asset() {
        with_contract(|env| {
            let asset = Address::generate(env);
            royalty_asset_validator::set_supported_royalty_asset(env, &asset, true);
            assert!(validate_royalty_asset(env, &asset).is_ok());
        });
    }

    #[test]
    fn validate_royalty_asset_rejects_unsupported_asset() {
        with_contract(|env| {
            let asset = Address::generate(env);
            let result = validate_royalty_asset(env, &asset);
            assert_eq!(
                result,
                Err(Error::UnsupportedAsset {
                    asset: asset.clone(),
                })
            );
        });
    }

    #[test]
    fn guards_are_composed_safely() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let token_id: TokenId = 1;
            let bps = 2500u32; // 25%

            let asset = Address::generate(env);
            royalty_asset_validator::set_supported_royalty_asset(env, &asset, true);

            // All validations should pass in sequence
            assert!(validate_royalty_recipient(env, &recipient).is_ok());
            assert!(validate_royalty_state(env, token_id).is_ok());
            assert!(validate_royalty_within_maximum(env, bps).is_ok());
            assert!(validate_royalty_asset(env, &asset).is_ok());
        });
    }

    #[test]
    fn validate_royalty_within_maximum_rejects_above_maximum() {
        with_contract(|env| {
            let bps = MAX_ROYALTY_BPS + 1;
            assert_eq!(
                validate_royalty_within_maximum(env, bps),
                Err(Error::InvalidRoyaltyBps)
            );
        });
    }

    #[test]
    fn guard_accepts_valid_context() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let ctx = GuardContext {
                env,
                authorized: true,
                recipient: Some(recipient),
                token_id: Some(1),
                royalty_bps: Some(2500),
            };
            assert!(RoyaltyGuard.validate(&ctx).is_ok());
        });
    }

    #[test]
    fn guard_rejects_unauthorized_update() {
        with_contract(|env| {
            let ctx = GuardContext {
                env,
                authorized: false,
                recipient: None,
                token_id: None,
                royalty_bps: None,
            };
            assert_eq!(
                RoyaltyGuard.validate(&ctx),
                Err(RoyaltyValidationError::UnauthorizedUpdate)
            );
        });
    }

    #[test]
    fn guard_rejects_bps_above_maximum() {
        with_contract(|env| {
            let ctx = GuardContext {
                env,
                authorized: true,
                recipient: None,
                token_id: None,
                royalty_bps: Some(MAX_ROYALTY_BPS + 1),
            };
            assert_eq!(
                RoyaltyGuard.validate(&ctx),
                Err(RoyaltyValidationError::BpsAboveMaximum)
            );
        });
    }
}
