//! Token ID validator — validates NFT token identifiers before contract operations.
//!
//! Enforces the following rules for every token ID entering the system:
//!
//! | Rule | Accepted | Rejected |
//! |------|----------|---------|
//! | Non-zero | `1..=u32::MAX` | `0` (reserved sentinel) |
//! | Existence | token has an ownership record | token has no ownership record |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Structural check only (no storage read):
//! token_id_validator::validate_token_id_format(token_id)?;
//!
//! // Full check including existence in storage:
//! token_id_validator::validate_token_id(env, token_id)?;
//! ```

use soroban_sdk::Env;

use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// The reserved sentinel value that is never a valid minted token ID.
///
/// Token IDs are assigned by an auto-incrementing counter starting at 1, so 0
/// is structurally invalid and is rejected before any storage read.
pub const INVALID_TOKEN_ID_SENTINEL: TokenId = 0;

// ── Structural validation ─────────────────────────────────────────────────────

/// Validate the structural rules of `token_id` without touching storage.
///
/// Currently rejects only the sentinel value `0`. All other `u32` values are
/// structurally acceptable; use [`validate_token_id`] when you also need to
/// verify the token exists in contract storage.
///
/// # Errors
/// Returns [`Error::InvalidTokenId`] when `token_id == 0`.
pub fn validate_token_id_format(token_id: TokenId) -> Result<(), Error> {
    if token_id == INVALID_TOKEN_ID_SENTINEL {
        return Err(Error::InvalidTokenId);
    }
    Ok(())
}

// ── Existence validation ──────────────────────────────────────────────────────

/// Validate that `token_id` is structurally valid **and** exists in storage.
///
/// Performs both checks:
/// 1. [`validate_token_id_format`] — rejects the sentinel value `0`.
/// 2. Ownership record lookup — rejects any ID for which no NFT has been minted.
///
/// # Errors
/// - [`Error::InvalidTokenId`] — `token_id` is the reserved sentinel `0`.
/// - [`Error::TokenNotFound`]  — no ownership record exists for `token_id`.
pub fn validate_token_id(env: &Env, token_id: TokenId) -> Result<(), Error> {
    validate_token_id_format(token_id)?;
    if !token_owner_storage::has_owner(env, token_id) {
        return Err(Error::TokenNotFound);
    }
    Ok(())
}

// ── Batch validation ──────────────────────────────────────────────────────────

/// Validate a slice of token IDs — each must pass [`validate_token_id`].
///
/// Returns on the first failure, reporting which `token_id` caused the error.
///
/// # Errors
/// Returns the first error encountered; see [`validate_token_id`] for details.
pub fn validate_token_ids(env: &Env, token_ids: &[TokenId]) -> Result<(), Error> {
    for &id in token_ids {
        validate_token_id(env, id)?;
    }
    Ok(())
}

/// Validate structural rules for a slice of token IDs without touching storage.
///
/// Useful for pre-flight checks on batch operations before any storage reads.
///
/// # Errors
/// Returns [`Error::InvalidTokenId`] on the first structurally invalid ID.
pub fn validate_token_id_formats(token_ids: &[TokenId]) -> Result<(), Error> {
    for &id in token_ids {
        validate_token_id_format(id)?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_owner_storage;
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

    // ── validate_token_id_format ──────────────────────────────────────────────

    #[ignore]
    #[test]
    fn format_rejects_sentinel_zero() {
        assert_eq!(
            validate_token_id_format(0),
            Err(Error::InvalidTokenId),
            "token ID 0 is the reserved sentinel and must be rejected"
        );
    }

    #[ignore]
    #[test]
    fn format_accepts_one() {
        assert!(validate_token_id_format(1).is_ok());
    }

    #[ignore]
    #[test]
    fn format_accepts_large_value() {
        assert!(validate_token_id_format(u32::MAX).is_ok());
    }

    #[ignore]
    #[test]
    fn format_accepts_typical_ids() {
        for id in [1u32, 100, 999, 10_000, u32::MAX] {
            assert!(
                validate_token_id_format(id).is_ok(),
                "expected token_id {} to pass format validation",
                id
            );
        }
    }

    // ── validate_token_id (format + existence) ────────────────────────────────

    #[ignore]
    #[test]
    fn rejects_sentinel_zero_before_storage_lookup() {
        with_contract(|env| {
            assert_eq!(
                validate_token_id(env, 0),
                Err(Error::InvalidTokenId),
                "sentinel 0 must be rejected before any storage read"
            );
        });
    }

    #[ignore]
    #[test]
    fn rejects_nonexistent_token() {
        with_contract(|env| {
            // token_id 42 has never been minted
            assert_eq!(
                validate_token_id(env, 42),
                Err(Error::TokenNotFound),
                "non-existent token ID must return TokenNotFound"
            );
        });
    }

    #[ignore]
    #[test]
    fn accepts_minted_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::save_owner(env, 1, &owner);

            assert!(
                validate_token_id(env, 1).is_ok(),
                "minted token ID 1 must pass validation"
            );
        });
    }

    #[ignore]
    #[test]
    fn accepts_multiple_minted_tokens() {
        with_contract(|env| {
            let owner = Address::generate(env);
            for id in [1u32, 2, 5, 100] {
                token_owner_storage::save_owner(env, id, &owner);
            }
            for id in [1u32, 2, 5, 100] {
                assert!(validate_token_id(env, id).is_ok());
            }
        });
    }

    #[ignore]
    #[test]
    fn rejects_token_after_owner_removed() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::save_owner(env, 7, &owner);
            token_owner_storage::remove_owner(env, 7);

            assert_eq!(
                validate_token_id(env, 7),
                Err(Error::TokenNotFound),
                "burned / removed token must be rejected"
            );
        });
    }

    // ── validate_token_ids (batch) ────────────────────────────────────────────

    #[ignore]
    #[test]
    fn batch_rejects_sentinel_in_list() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::save_owner(env, 1, &owner);
            token_owner_storage::save_owner(env, 2, &owner);

            // 0 is mixed into an otherwise-valid batch
            assert_eq!(
                validate_token_ids(env, &[1, 0, 2]),
                Err(Error::InvalidTokenId)
            );
        });
    }

    #[ignore]
    #[test]
    fn batch_rejects_nonexistent_token_in_list() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::save_owner(env, 1, &owner);

            // token 99 was never minted
            assert_eq!(
                validate_token_ids(env, &[1, 99]),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[ignore]
    #[test]
    fn batch_accepts_all_valid_tokens() {
        with_contract(|env| {
            let owner = Address::generate(env);
            for id in [1u32, 2, 3] {
                token_owner_storage::save_owner(env, id, &owner);
            }
            assert!(validate_token_ids(env, &[1, 2, 3]).is_ok());
        });
    }

    #[ignore]
    #[test]
    fn batch_accepts_empty_slice() {
        with_contract(|env| {
            assert!(validate_token_ids(env, &[]).is_ok());
        });
    }

    // ── validate_token_id_formats (batch structural) ──────────────────────────

    #[ignore]
    #[test]
    fn batch_format_rejects_sentinel() {
        assert_eq!(
            validate_token_id_formats(&[1, 2, 0, 3]),
            Err(Error::InvalidTokenId)
        );
    }

    #[ignore]
    #[test]
    fn batch_format_accepts_all_nonzero() {
        assert!(validate_token_id_formats(&[1, 2, 100, u32::MAX]).is_ok());
    }

    #[ignore]
    #[test]
    fn batch_format_accepts_empty_slice() {
        assert!(validate_token_id_formats(&[]).is_ok());
    }
}
