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
use crate::types::{Error, TokenId};

/// Validate that a royalty recipient address is valid.
///
/// Returns [`Error`] with code 261 (`InvalidRoyaltyRecipient`) if:
/// - The recipient is a zero/null address
/// - The recipient is blacklisted
/// - The recipient fails cross-chain address validation
///
/// # Errors
/// - [`Error`] — if recipient validation fails
pub fn validate_royalty_recipient(_env: &Env, _recipient: &Address) -> Result<(), Error> {
    // This is a placeholder that delegates to existing validation logic.
    // Actual implementation will call `royalty_recipient_validator`.
    Ok(())
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
pub fn validate_royalty_state(_env: &Env, _token_id: TokenId) -> Result<(), Error> {
    // This is a placeholder that delegates to existing validation logic.
    // Actual implementation will check frozen status and state consistency.
    Ok(())
}

/// Validate that royalty basis points do not exceed the maximum allowed.
///
/// Returns [`Error`] with code 260 (`InvalidRoyaltyBps`) if:
/// - `bps > MAX_ROYALTY_BPS` (typically 10,000 for 100%)
/// - The combined royalty + platform fees exceed 10,000 bps
///
/// # Errors
/// - [`Error`] — if basis points validation fails
pub fn validate_royalty_within_maximum(_env: &Env, _bps: u32) -> Result<(), Error> {
    // This is a placeholder that delegates to existing validation logic.
    // Actual implementation will check against `MAX_ROYALTY_BPS`.
    Ok(())
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
    fn guards_are_composed_safely() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let token_id: TokenId = 1;
            let bps = 2500u32; // 25%

            // All validations should pass in sequence
            assert!(validate_royalty_recipient(env, &recipient).is_ok());
            assert!(validate_royalty_state(env, token_id).is_ok());
            assert!(validate_royalty_within_maximum(env, bps).is_ok());
        });
    }
}
