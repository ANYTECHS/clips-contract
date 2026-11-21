//! NFT existence validation guard (Issue #1093).
//!
//! Implements a guard that verifies an NFT exists before contract operations are
//! performed on it.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Check token existence | [`check_token_exists`] |
//! | Reject nonexistent tokens | [`require_token_exists`] |
//! | Use centralized token-not-found error | [`TokenNotFoundError`] |
//! | Add unit tests | Comprehensive test module |
//!
//! # Error handling
//!
//! All functions return the centralized [`TokenNotFoundError`] (error code 230)
//! from the error catalog module. This ensures consistent error reporting across
//! the contract.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full guard with error handling
//! nft_existence_guard::require_token_exists(&env, token_id)?;
//!
//! // Check existence without error
//! if nft_existence_guard::check_token_exists(&env, token_id) {
//!     // token exists; safe to proceed
//! }
//! ```

use soroban_sdk::Env;

use crate::guard_interface::{Guard, GuardContext};
use crate::guard_result::GuardResult;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Primary integration point ────────────────────────────────────────────────

/// Require that `token_id` exists before proceeding.
///
/// Returns `Ok(())` if the token exists, or [`TokenNotFoundError`] if it does not.
///
/// This is the authoritative guard for NFT existence checks. Use it at the start
/// of any entry point that operates on an NFT.
///
/// # Errors
/// - [`TokenNotFoundError`] — token does not exist in storage.
///
/// # Example
///
/// ```rust,ignore
/// nft_existence_guard::require_token_exists(&env, token_id)?;
/// // If Ok, token is guaranteed to exist; safe to proceed
/// ```
pub fn require_token_exists(env: &Env, token_id: TokenId) -> Result<(), crate::error_catalog::TokenNotFoundError> {
    if check_token_exists(env, token_id) {
        Ok(())
    } else {
        Err(crate::error_catalog::TokenNotFoundError::TokenNotFound)
    }
}

// ─── Individual existence probes ───────────────────────────────────────────────

/// Return `true` when `token_id` exists in storage.
///
/// Performs no error handling—purely a read-only check. Use this when you need
/// a boolean result or are building custom error handling logic.
///
/// A token is considered to exist when an ownership record is stored for it.
///
/// # Example
///
/// ```rust,ignore
/// if nft_existence_guard::check_token_exists(&env, token_id) {
///     // token exists in storage
/// }
/// ```
pub fn check_token_exists(env: &Env, token_id: TokenId) -> bool {
    token_owner_storage::has_owner(env, token_id)
}

// ─── Standardized guard adapter (issue #1414) ────────────────────────────────

/// Guard that verifies an NFT exists before an operation is performed on it.
///
/// The token id is read from [`GuardContext::token_id`]. A context without a
/// token id is rejected with the same error as a missing token, because there
/// is no NFT that could satisfy the pre-condition.
///
/// # Errors
///
/// - [`Error::TokenNotFound`] — the token does not exist (or no token id was
///   supplied). This is the centralized token-not-found error of the contract
///   error registry; the catalog equivalent is
///   [`crate::error_catalog::TokenNotFoundError`].
#[derive(Clone, Copy, Debug, Default)]
pub struct TokenExistenceGuard;

impl Guard for TokenExistenceGuard {
    fn execute(&self, ctx: &GuardContext) -> GuardResult {
        let token_id = match ctx.token_id {
            Some(id) => id,
            None => return GuardResult::Fail(Error::TokenNotFound),
        };

        if check_token_exists(&ctx.env, token_id) {
            GuardResult::Pass
        } else {
            GuardResult::Fail(Error::TokenNotFound)
        }
    }

    fn name(&self) -> &'static str {
        "token_existence_guard"
    }
}

/// Guard an operation with [`TokenExistenceGuard`] using the centralized error.
///
/// Returns the catalog-level [`crate::error_catalog::TokenNotFoundError`] so
/// callers that use the centralized error catalog keep its numeric code (230).
pub fn require_token_exists_for_context(
    ctx: &GuardContext,
) -> Result<(), crate::error_catalog::TokenNotFoundError> {
    let token_id = ctx
        .token_id
        .ok_or(crate::error_catalog::TokenNotFoundError::TokenNotFound)?;
    crate::error_catalog::TokenNotFoundError::require_token_exists(check_token_exists(
        &ctx.env, token_id,
    ))
}

/// Require token existence via the standardized guard framework.
pub fn guard_token_exists(ctx: &GuardContext) -> Result<(), Error> {
    crate::guard_execution_helper::execute(ctx, &TokenExistenceGuard)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ───────────────────────────────────────────────────────

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    // ── require_token_exists ───────────────────────────────────────────────

    #[test]
    fn require_token_exists_passes_for_existing_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(require_token_exists(env, token_id).is_ok());
        });
    }

    #[test]
    fn require_token_exists_fails_for_nonexistent_token() {
        with_contract(|env| {
            let nonexistent_token = 999;

            assert_eq!(
                require_token_exists(env, nonexistent_token),
                Err(crate::error_catalog::TokenNotFoundError::TokenNotFound)
            );
        });
    }

    // ── check_token_exists ─────────────────────────────────────────────────

    #[test]
    fn check_token_exists_returns_true_for_existing_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(check_token_exists(env, token_id));
        });
    }

    #[test]
    fn check_token_exists_returns_false_for_nonexistent_token() {
        with_contract(|env| {
            let nonexistent_token = 999;

            assert!(!check_token_exists(env, nonexistent_token));
        });
    }

    // ── Multiple tokens ────────────────────────────────────────────────────

    #[test]
    fn check_token_exists_distinguishes_between_tokens() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);
            setup_token(env, 5, &owner);

            assert!(check_token_exists(env, 1));
            assert!(check_token_exists(env, 2));
            assert!(check_token_exists(env, 5));
            assert!(!check_token_exists(env, 3));
            assert!(!check_token_exists(env, 4));
            assert!(!check_token_exists(env, 6));
        });
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn check_token_exists_is_deterministic_across_calls() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            // Multiple calls should return consistent results
            assert!(check_token_exists(env, token_id));
            assert!(check_token_exists(env, token_id));
            assert!(check_token_exists(env, token_id));
        });
    }

    #[test]
    fn check_token_exists_handles_token_id_zero() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 0, &owner);

            assert!(check_token_exists(env, 0));
        });
    }

    #[test]
    fn check_token_exists_handles_large_token_ids() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let large_id = u64::MAX;
            setup_token(env, large_id, &owner);

            assert!(check_token_exists(env, large_id));
            assert!(!check_token_exists(env, large_id - 1));
        });
    }

    // ── Centralized error code verification ────────────────────────────────

    #[test]
    fn require_token_exists_uses_token_not_found_error() {
        with_contract(|env| {
            let nonexistent = 999;
            let error = require_token_exists(env, nonexistent).unwrap_err();

            // Verify it's the TokenNotFound variant
            assert_eq!(error, crate::error_catalog::TokenNotFoundError::TokenNotFound);
            // Verify the error code matches the catalog
            assert_eq!(error.code(), crate::error_catalog::TokenNotFoundError::CODE);
        });
    }

    #[test]
    fn token_not_found_error_has_correct_code() {
        assert_eq!(crate::error_catalog::TokenNotFoundError::CODE, 230);
    }

    // ── Consistency with centralized error catalog ──────────────────────────

    #[test]
    fn error_code_matches_centralized_registry() {
        with_contract(|env| {
            let _ = require_token_exists(env, 999);
            // Code 230 is from the centralized error registry (error_catalog)
            assert_eq!(crate::error_catalog::TokenNotFoundError::CODE, 230);
            assert_eq!(crate::error_catalog::TokenNotFoundError::NAME, "TokenNotFound");
        });
    }

    // ── TokenExistenceGuard (issue #1414) ──────────────────────────────────────

    #[test]
    fn token_existence_guard_passes_for_existing_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 7;
            setup_token(env, token_id, &owner);

            let ctx = crate::guard_interface::GuardContext::with_token(
                env.clone(),
                owner,
                token_id,
            );
            assert_eq!(TokenExistenceGuard.execute(&ctx), GuardResult::Pass);
            assert_eq!(guard_token_exists(&ctx), Ok(()));
            assert_eq!(TokenExistenceGuard.name(), "token_existence_guard");
        });
    }

    #[test]
    fn token_existence_guard_rejects_nonexistent_token() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx =
                crate::guard_interface::GuardContext::with_token(env.clone(), caller, 999);

            assert_eq!(
                TokenExistenceGuard.execute(&ctx),
                GuardResult::Fail(Error::TokenNotFound)
            );
            assert_eq!(guard_token_exists(&ctx), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn token_existence_guard_rejects_context_without_token_id() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx = crate::guard_interface::GuardContext::new(env.clone(), caller);

            assert_eq!(
                TokenExistenceGuard.execute(&ctx),
                GuardResult::Fail(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn require_token_exists_for_context_uses_centralized_error() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx =
                crate::guard_interface::GuardContext::with_token(env.clone(), caller, 999);

            let error = require_token_exists_for_context(&ctx).unwrap_err();
            assert_eq!(
                error,
                crate::error_catalog::TokenNotFoundError::TokenNotFound
            );
            assert_eq!(error.code(), crate::error_catalog::TokenNotFoundError::CODE);
        });
    }
}
