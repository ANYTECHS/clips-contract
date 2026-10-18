//! Ownership authorization guard (Issue #1089).
//!
//! Verifies that the caller owns the NFT or resource being modified.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Retrieve resource ownership | [`get_owner_for_token`] |
//! | Compare owner with caller | [`check_caller_is_owner`] |
//! | Reject non-owners | [`require_owner`] |
//! | Add ownership authorization tests | Tests module with comprehensive coverage |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full guard with auth requirement
//! ownership_guard::require_owner(&env, &caller, token_id)?;
//!
//! // Check ownership without auth
//! if ownership_guard::check_caller_is_owner(&env, &caller, token_id) {
//!     // caller owns the token
//! }
//!
//! // Get the owner
//! let owner = ownership_guard::get_owner_for_token(&env, token_id)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Primary integration point ────────────────────────────────────────────────

/// Require that `caller` owns `token_id`.
///
/// Verifies ownership and demands an authorization signature from the caller
/// via [`Address::require_auth`]. If the caller is not the owner, returns
/// [`Error::Unauthorized`]. If the token does not exist, returns
/// [`Error::TokenNotFound`].
///
/// This is the authoritative guard for ownership checks. Use it whenever you
/// need to verify both ownership **and** authorize a sensitive operation.
///
/// # Errors
/// - [`Error::TokenNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — caller is not the token owner.
pub fn require_owner(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    // Demand a signed authorization envelope from the caller first
    caller.require_auth();

    if check_caller_is_owner(env, caller, token_id) {
        return Ok(());
    }

    // Determine which error to return
    if token_owner_storage::has_owner(env, token_id) {
        Err(Error::Unauthorized)
    } else {
        Err(Error::TokenNotFound)
    }
}

// ─── Individual authorization probes ─────────────────────────────────────────

/// Return `true` when `caller` is the owner of `token_id`.
///
/// Performs no authorization call and no error handling—purely a read-only
/// check. Use this when you already have the required authorization or need
/// to make a non-authoritative probe.
///
/// Returns `false` if:
/// - The token does not exist, or
/// - `caller` is not the owner.
///
/// # Example
///
/// ```rust,ignore
/// if ownership_guard::check_caller_is_owner(&env, &caller, token_id) {
///     // caller owns the token; safe to proceed
/// }
/// ```
pub fn check_caller_is_owner(env: &Env, caller: &Address, token_id: TokenId) -> bool {
    token_owner_storage::get_owner(env, token_id)
        .map(|owner| owner == *caller)
        .unwrap_or(false)
}

/// Retrieve the owner of `token_id`.
///
/// Returns the address that currently owns `token_id`, or `TokenNotFound` if
/// the token does not exist.
///
/// # Errors
/// - [`Error::TokenNotFound`] — token does not exist.
///
/// # Example
///
/// ```rust,ignore
/// let owner = ownership_guard::get_owner_for_token(&env, token_id)?;
/// if owner == some_expected_address {
///     // ownership matches expected
/// }
/// ```
pub fn get_owner_for_token(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    token_owner_storage::get_owner(env, token_id)
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

    // ── require_owner ──────────────────────────────────────────────────────

    #[test]
    fn require_owner_accepts_the_actual_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(require_owner(env, &owner, token_id).is_ok());
        });
    }

    #[test]
    fn require_owner_rejects_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert_eq!(
                require_owner(env, &other, token_id),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_owner_rejects_when_token_not_found() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let nonexistent_token = 999;

            assert_eq!(
                require_owner(env, &caller, nonexistent_token),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── check_caller_is_owner ──────────────────────────────────────────────

    #[test]
    fn check_caller_is_owner_returns_true_for_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(check_caller_is_owner(env, &owner, token_id));
        });
    }

    #[test]
    fn check_caller_is_owner_returns_false_for_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(!check_caller_is_owner(env, &other, token_id));
        });
    }

    #[test]
    fn check_caller_is_owner_returns_false_for_nonexistent_token() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let nonexistent_token = 999;

            assert!(!check_caller_is_owner(env, &caller, nonexistent_token));
        });
    }

    // ── get_owner_for_token ────────────────────────────────────────────────

    #[test]
    fn get_owner_for_token_returns_the_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            let retrieved = get_owner_for_token(env, token_id).unwrap();
            assert_eq!(retrieved, owner);
        });
    }

    #[test]
    fn get_owner_for_token_returns_not_found_for_missing_token() {
        with_contract(|env| {
            let nonexistent_token = 999;

            assert_eq!(
                get_owner_for_token(env, nonexistent_token),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn ownership_check_is_deterministic_across_multiple_calls() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            // Call multiple times; result should be consistent
            assert!(check_caller_is_owner(env, &owner, token_id));
            assert!(check_caller_is_owner(env, &owner, token_id));
            assert!(check_caller_is_owner(env, &owner, token_id));
        });
    }

    #[test]
    fn multiple_tokens_maintain_independent_ownership() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);

            setup_token(env, 1, &owner_a);
            setup_token(env, 2, &owner_b);

            assert!(check_caller_is_owner(env, &owner_a, 1));
            assert!(!check_caller_is_owner(env, &owner_a, 2));
            assert!(!check_caller_is_owner(env, &owner_b, 1));
            assert!(check_caller_is_owner(env, &owner_b, 2));
        });
    }
}
