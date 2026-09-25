//! Owner validation guard (Issue #1095).
//!
//! Implements a guard that validates the current owner of an NFT.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Retrieve current NFT owner | [`get_current_owner`] |
//! | Compare against expected owner | [`check_owner_matches`] |
//! | Reject mismatched ownership | [`require_owner_matches`] |
//! | Add ownership tests | Comprehensive test module |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full guard with error handling
//! owner_validation_guard::require_owner_matches(&env, token_id, &expected_owner)?;
//!
//! // Check without error
//! if owner_validation_guard::check_owner_matches(&env, token_id, &expected_owner) {
//!     // owner matches expected
//! }
//!
//! // Retrieve the current owner
//! let owner = owner_validation_guard::get_current_owner(&env, token_id)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::token_owner_storage;
use crate::types::TokenId;

// ─── Primary integration point ────────────────────────────────────────────────

/// Require that the current owner of `token_id` matches `expected_owner`.
///
/// Returns `Ok(())` if the current owner matches the expected owner, or an error
/// if they do not match or the token does not exist.
///
/// This is the authoritative guard for ownership validation. Use it when you
/// need to verify that a specific address currently owns a token (e.g., before
/// accepting a transfer, before minting royalty payments).
///
/// # Errors
/// - [`crate::types::Error::TokenNotFound`] — token does not exist.
/// - [`crate::types::Error::Unauthorized`] — current owner does not match expected owner.
///
/// # Example
///
/// ```rust,ignore
/// owner_validation_guard::require_owner_matches(&env, token_id, &seller)?;
/// // If Ok, seller is verified as the current owner
/// ```
pub fn require_owner_matches(
    env: &Env,
    token_id: TokenId,
    expected_owner: &Address,
) -> Result<(), crate::types::Error> {
    let current = get_current_owner(env, token_id)?;
    if check_owner_matches_internal(&current, expected_owner) {
        return Ok(());
    }
    Err(crate::types::Error::Unauthorized)
}

// ─── Individual ownership probes ───────────────────────────────────────────────

/// Return `true` when the current owner of `token_id` matches `expected_owner`.
///
/// Performs no error handling—purely a read-only check. Use this when you need
/// a boolean result or are building custom error handling logic.
///
/// Returns `false` if:
/// - The token does not exist, or
/// - The current owner differs from the expected owner.
///
/// # Example
///
/// ```rust,ignore
/// if owner_validation_guard::check_owner_matches(&env, token_id, &expected_owner) {
///     // owner matches; safe to proceed
/// }
/// ```
pub fn check_owner_matches(
    env: &Env,
    token_id: TokenId,
    expected_owner: &Address,
) -> bool {
    token_owner_storage::get_owner(env, token_id)
        .map(|current| check_owner_matches_internal(&current, expected_owner))
        .unwrap_or(false)
}

/// Retrieve the current owner of `token_id`.
///
/// Returns the address that currently owns `token_id`, or an error if the token
/// does not exist.
///
/// # Errors
/// - [`crate::types::Error::TokenNotFound`] — token does not exist.
///
/// # Example
///
/// ```rust,ignore
/// let current_owner = owner_validation_guard::get_current_owner(&env, token_id)?;
/// if current_owner == some_expected_address {
///     // ownership matches
/// }
/// ```
pub fn get_current_owner(env: &Env, token_id: TokenId) -> Result<Address, crate::types::Error> {
    token_owner_storage::get_owner(env, token_id)
}

// ─── Internal helpers ──────────────────────────────────────────────────────────

/// Pure comparison helper: return `true` if the addresses match.
#[inline]
fn check_owner_matches_internal(current: &Address, expected: &Address) -> bool {
    current == expected
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

    // ── require_owner_matches ──────────────────────────────────────────────

    #[test]
    fn require_owner_matches_passes_when_owner_matches() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(require_owner_matches(env, token_id, &owner).is_ok());
        });
    }

    #[test]
    fn require_owner_matches_fails_when_owner_differs() {
        with_contract(|env| {
            let current_owner = Address::generate(env);
            let expected_owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &current_owner);

            assert_eq!(
                require_owner_matches(env, token_id, &expected_owner),
                Err(crate::types::Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_owner_matches_fails_when_token_not_found() {
        with_contract(|env| {
            let expected_owner = Address::generate(env);
            let nonexistent_token = 999;

            assert_eq!(
                require_owner_matches(env, nonexistent_token, &expected_owner),
                Err(crate::types::Error::TokenNotFound)
            );
        });
    }

    // ── check_owner_matches ────────────────────────────────────────────────

    #[test]
    fn check_owner_matches_returns_true_when_owner_matches() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(check_owner_matches(env, token_id, &owner));
        });
    }

    #[test]
    fn check_owner_matches_returns_false_when_owner_differs() {
        with_contract(|env| {
            let current_owner = Address::generate(env);
            let expected_owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &current_owner);

            assert!(!check_owner_matches(env, token_id, &expected_owner));
        });
    }

    #[test]
    fn check_owner_matches_returns_false_when_token_not_found() {
        with_contract(|env| {
            let expected_owner = Address::generate(env);
            let nonexistent_token = 999;

            assert!(!check_owner_matches(env, nonexistent_token, &expected_owner));
        });
    }

    // ── get_current_owner ──────────────────────────────────────────────────

    #[test]
    fn get_current_owner_returns_the_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            let retrieved = get_current_owner(env, token_id).unwrap();
            assert_eq!(retrieved, owner);
        });
    }

    #[test]
    fn get_current_owner_fails_when_token_not_found() {
        with_contract(|env| {
            let nonexistent_token = 999;

            assert_eq!(
                get_current_owner(env, nonexistent_token),
                Err(crate::types::Error::TokenNotFound)
            );
        });
    }

    // ── Multiple tokens ────────────────────────────────────────────────────

    #[test]
    fn check_owner_matches_distinguishes_between_tokens() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let owner_c = Address::generate(env);

            setup_token(env, 1, &owner_a);
            setup_token(env, 2, &owner_b);
            setup_token(env, 3, &owner_c);

            assert!(check_owner_matches(env, 1, &owner_a));
            assert!(!check_owner_matches(env, 1, &owner_b));
            assert!(!check_owner_matches(env, 1, &owner_c));

            assert!(!check_owner_matches(env, 2, &owner_a));
            assert!(check_owner_matches(env, 2, &owner_b));
            assert!(!check_owner_matches(env, 2, &owner_c));

            assert!(!check_owner_matches(env, 3, &owner_a));
            assert!(!check_owner_matches(env, 3, &owner_b));
            assert!(check_owner_matches(env, 3, &owner_c));
        });
    }

    // ── Multiple owners validation ─────────────────────────────────────────

    #[test]
    fn get_current_owner_retrieves_correct_owner_for_each_token() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);

            setup_token(env, 1, &owner_a);
            setup_token(env, 2, &owner_b);

            assert_eq!(get_current_owner(env, 1).unwrap(), owner_a);
            assert_eq!(get_current_owner(env, 2).unwrap(), owner_b);
        });
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn owner_validation_is_deterministic_across_calls() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            // Multiple calls should return consistent results
            assert!(check_owner_matches(env, token_id, &owner));
            assert!(check_owner_matches(env, token_id, &owner));
            assert!(check_owner_matches(env, token_id, &owner));
        });
    }

    #[test]
    fn require_owner_matches_is_deterministic_across_calls() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            // Multiple calls with same parameters should return same result
            assert!(require_owner_matches(env, token_id, &owner).is_ok());
            assert!(require_owner_matches(env, token_id, &owner).is_ok());
            assert!(require_owner_matches(env, token_id, &owner).is_ok());
        });
    }

    // ── Comparison semantics ───────────────────────────────────────────────

    #[test]
    fn check_owner_matches_uses_address_equality() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            // Same address object should match
            assert!(check_owner_matches(env, token_id, &owner));

            // Creating a reference from the same owner should also match
            let owner_ref = &owner;
            assert!(check_owner_matches(env, token_id, owner_ref));
        });
    }

    // ── Error type verification ────────────────────────────────────────────

    #[test]
    fn require_owner_matches_returns_correct_error_for_nonexistent_token() {
        with_contract(|env| {
            let expected_owner = Address::generate(env);
            let error = require_owner_matches(env, 999, &expected_owner).unwrap_err();
            assert_eq!(error, crate::types::Error::TokenNotFound);
        });
    }

    #[test]
    fn require_owner_matches_returns_correct_error_for_mismatched_owner() {
        with_contract(|env| {
            let current_owner = Address::generate(env);
            let different_owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &current_owner);

            let error = require_owner_matches(env, token_id, &different_owner).unwrap_err();
            assert_eq!(error, crate::types::Error::Unauthorized);
        });
    }
}
