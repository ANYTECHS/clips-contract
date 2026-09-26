//! Token existence validator (issue #1095).
//!
//! Validates that an NFT exists before an operation is performed against it.
//!
//! The crate already had two existence checks, and they do not read the same
//! place:
//!
//! * [`token_storage::token_exists`] — the `Token(token_id)` record, written by
//!   [`token_storage::set_token`].
//! * [`error_catalog::ensure_token_exists`] — the `Owner(token_id)` record,
//!   i.e. [`token_owner_storage::has_owner`].
//!
//! A token minted through a path that writes one record and not the other is
//! therefore "existing" for half the codebase. This validator is the single
//! entry point that requires both, and it reports the failure with the
//! centralized [`TokenNotFoundError`] (code 230) rather than a bare bool:
//!
//! ```ignore
//! token_existence_validator::validate_token_exists(&env, token_id)?;
//! ```
//!
//! Entry points that must return the contract's own error enum use
//! [`validate_token_exists_as_contract_error`], which is the same check mapped
//! to [`Error::TokenNotFound`].

use soroban_sdk::Env;

use crate::error_catalog::{ensure_token_exists, TokenNotFoundError};
use crate::token_storage;
use crate::types::{Error, TokenId};

/// `true` when both the token record and the ownership record are present.
///
/// Prefer [`validate_token_exists`] when the caller needs to know *why* the
/// answer is `false`.
pub fn token_exists(env: &Env, token_id: TokenId) -> bool {
    token_storage::token_exists(env, token_id)
        && crate::token_owner_storage::has_owner(env, token_id)
}

/// Reject a token that does not exist, using the centralized token-not-found
/// error.
///
/// # Errors
/// * [`TokenNotFoundError::TokenNotFound`] — the token record, the ownership
///   record, or both are missing.
pub fn validate_token_exists(env: &Env, token_id: TokenId) -> Result<(), TokenNotFoundError> {
    if !token_storage::token_exists(env, token_id) {
        return Err(TokenNotFoundError::TokenNotFound);
    }
    // Ownership record, reported with the same centralized error.
    ensure_token_exists(env, token_id)
}

/// The same check, expressed with the contract's own [`Error`].
///
/// # Errors
/// * [`Error::TokenNotFound`] — see [`validate_token_exists`].
pub fn validate_token_exists_as_contract_error(env: &Env, token_id: TokenId) -> Result<(), Error> {
    token_storage::require_token_exists(env, token_id)?;
    if !crate::token_owner_storage::has_owner(env, token_id) {
        return Err(Error::TokenNotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_owner_storage;
    use crate::types::TokenData;
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

    fn save_token_record(env: &Env, token_id: TokenId, owner: &Address) {
        token_storage::set_token(
            env,
            token_id,
            &TokenData {
                owner: owner.clone(),
                clip_id: 1,
            },
        );
    }

    #[test]
    fn nonexistent_token_is_rejected_with_the_centralized_error() {
        with_contract(|env| {
            let result = validate_token_exists(env, 404);
            assert_eq!(result, Err(TokenNotFoundError::TokenNotFound));
            // The code the centralized registry assigns to this error.
            assert_eq!(TokenNotFoundError::TokenNotFound.code(), 230);
        });
    }

    #[test]
    fn nonexistent_token_is_rejected_as_a_contract_error_too() {
        with_contract(|env| {
            assert_eq!(
                validate_token_exists_as_contract_error(env, 404),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn token_record_without_owner_record_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_token_record(env, 7, &owner);

            // The storage check alone would accept this token...
            assert!(token_storage::token_exists(env, 7));
            // ...but nothing owns it, so operations against it are unsafe.
            assert_eq!(
                validate_token_exists(env, 7),
                Err(TokenNotFoundError::TokenNotFound)
            );
            assert_eq!(
                validate_token_exists_as_contract_error(env, 7),
                Err(Error::TokenNotFound)
            );
            assert!(!token_exists(env, 7));
        });
    }

    #[test]
    fn owner_record_without_token_record_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::save_owner(env, 8, &owner);

            // `ensure_token_exists` alone would accept this token.
            assert!(error_catalog_accepts(env, 8));
            assert_eq!(
                validate_token_exists(env, 8),
                Err(TokenNotFoundError::TokenNotFound)
            );
            assert_eq!(
                validate_token_exists_as_contract_error(env, 8),
                Err(Error::TokenNotFound)
            );
        });
    }

    fn error_catalog_accepts(env: &Env, token_id: TokenId) -> bool {
        ensure_token_exists(env, token_id).is_ok()
    }

    #[test]
    fn token_with_both_records_is_accepted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_token_record(env, 9, &owner);
            token_owner_storage::save_owner(env, 9, &owner);

            assert!(token_exists(env, 9));
            assert_eq!(validate_token_exists(env, 9), Ok(()));
            assert_eq!(validate_token_exists_as_contract_error(env, 9), Ok(()));
        });
    }

    #[test]
    fn removing_the_token_record_makes_an_owned_token_invalid_again() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_token_record(env, 10, &owner);
            token_owner_storage::save_owner(env, 10, &owner);
            assert_eq!(validate_token_exists(env, 10), Ok(()));

            token_storage::remove_token(env, 10);

            assert_eq!(
                validate_token_exists(env, 10),
                Err(TokenNotFoundError::TokenNotFound)
            );
            // The ownership record is still there, which is exactly the state
            // the two pre-existing helpers disagreed about.
            assert!(token_owner_storage::has_owner(env, 10));
        });
    }
}
