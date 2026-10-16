//! Creator authorization storage — tracks explicitly authorized delegate accounts
//! per token (issue #1249).
//!
//! The registered creator of a token is the canonical authority for
//! creator-specific operations.  This module lets the creator (or an admin)
//! designate a single additional account — a **delegate** — that is permitted
//! to perform those same operations on a given token.
//!
//! # Storage layout
//! Key:   `DataKey::CreatorAuthorized(token_id)` (persistent storage)
//! Value: `Address` — the currently authorized delegate.
//!
//! Only one delegate per token is supported.  Writing a new delegate
//! overwrites the previous one.  Revocation removes the key entirely.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Authorize a delegate for token 7:
//! creator_auth_storage::set_authorized_account(&env, 7, &delegate);
//!
//! // Check whether an address is the current delegate:
//! let is_auth = creator_auth_storage::is_authorized_account(&env, 7, &caller);
//!
//! // Revoke the delegation:
//! creator_auth_storage::remove_authorized_account(&env, 7);
//! ```

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

// ─── Write ────────────────────────────────────────────────────────────────────

/// Authorize `account` to act as creator delegate for `token_id`.
///
/// Calling this function when a delegate is already set **silently overwrites**
/// the existing delegate.  Callers that need to enforce a "no existing
/// delegate" invariant should check [`has_authorized_account`] first.
pub fn set_authorized_account(env: &Env, token_id: TokenId, account: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::CreatorAuthorized(token_id), account);
}

/// Remove any existing delegate authorization for `token_id`.
///
/// This is a no-op when no delegate is currently set.
pub fn remove_authorized_account(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::CreatorAuthorized(token_id));
}

// ─── Read ─────────────────────────────────────────────────────────────────────

/// Return the authorized delegate for `token_id`, or `None` if none is set.
pub fn get_authorized_account(env: &Env, token_id: TokenId) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorAuthorized(token_id))
}

/// Return `true` when `account` is the currently authorized delegate for
/// `token_id`.
///
/// Returns `false` when no delegate is set or when the set delegate differs
/// from `account`.
pub fn is_authorized_account(env: &Env, token_id: TokenId, account: &Address) -> bool {
    match get_authorized_account(env, token_id) {
        Some(delegate) => delegate == *account,
        None => false,
    }
}

/// Return `true` when any delegate has been authorized for `token_id`.
pub fn has_authorized_account(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::CreatorAuthorized(token_id))
}

// ─── Result-returning helpers ─────────────────────────────────────────────────

/// Return the authorized delegate for `token_id`, or
/// `Err(Error::TokenNotFound)` if none has been set.
///
/// Prefer [`get_authorized_account`] (returns `Option`) when absence of a
/// delegate is a valid non-error state.
pub fn require_authorized_account(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    get_authorized_account(env, token_id).ok_or(Error::TokenNotFound)
}

// ─────────────────────────────────────────────────────────────────────────────
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

    // ── set / get ─────────────────────────────────────────────────────────────

    #[test]
    fn set_and_get_authorized_account() {
        with_contract(|env| {
            let delegate = Address::generate(env);
            set_authorized_account(env, 1, &delegate);
            assert_eq!(get_authorized_account(env, 1), Some(delegate));
        });
    }

    #[test]
    fn get_returns_none_when_unset() {
        with_contract(|env| {
            assert_eq!(get_authorized_account(env, 99), None);
        });
    }

    #[test]
    fn set_overwrites_existing_delegate() {
        with_contract(|env| {
            let first = Address::generate(env);
            let second = Address::generate(env);
            set_authorized_account(env, 1, &first);
            set_authorized_account(env, 1, &second);
            assert_eq!(get_authorized_account(env, 1), Some(second));
        });
    }

    // ── is_authorized_account ─────────────────────────────────────────────────

    #[test]
    fn is_authorized_returns_true_for_matching_delegate() {
        with_contract(|env| {
            let delegate = Address::generate(env);
            set_authorized_account(env, 1, &delegate);
            assert!(is_authorized_account(env, 1, &delegate));
        });
    }

    #[test]
    fn is_authorized_returns_false_for_different_address() {
        with_contract(|env| {
            let delegate = Address::generate(env);
            let other = Address::generate(env);
            set_authorized_account(env, 1, &delegate);
            assert!(!is_authorized_account(env, 1, &other));
        });
    }

    #[test]
    fn is_authorized_returns_false_when_no_delegate_set() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(!is_authorized_account(env, 1, &caller));
        });
    }

    // ── has_authorized_account ────────────────────────────────────────────────

    #[test]
    fn has_authorized_returns_true_after_set() {
        with_contract(|env| {
            let delegate = Address::generate(env);
            set_authorized_account(env, 1, &delegate);
            assert!(has_authorized_account(env, 1));
        });
    }

    #[test]
    fn has_authorized_returns_false_when_unset() {
        with_contract(|env| {
            assert!(!has_authorized_account(env, 42));
        });
    }

    // ── remove ────────────────────────────────────────────────────────────────

    #[test]
    fn remove_clears_delegate() {
        with_contract(|env| {
            let delegate = Address::generate(env);
            set_authorized_account(env, 1, &delegate);
            remove_authorized_account(env, 1);
            assert!(!has_authorized_account(env, 1));
            assert_eq!(get_authorized_account(env, 1), None);
        });
    }

    #[test]
    fn remove_when_no_delegate_is_noop() {
        with_contract(|env| {
            // must not panic
            remove_authorized_account(env, 99);
            assert!(!has_authorized_account(env, 99));
        });
    }

    // ── require_authorized_account ────────────────────────────────────────────

    #[test]
    fn require_returns_delegate_when_set() {
        with_contract(|env| {
            let delegate = Address::generate(env);
            set_authorized_account(env, 1, &delegate);
            assert_eq!(require_authorized_account(env, 1), Ok(delegate));
        });
    }

    #[test]
    fn require_returns_token_not_found_when_absent() {
        with_contract(|env| {
            assert_eq!(
                require_authorized_account(env, 55),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── per-token isolation ───────────────────────────────────────────────────

    #[test]
    fn delegates_are_isolated_per_token() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            set_authorized_account(env, 1, &a);
            set_authorized_account(env, 2, &b);

            assert_eq!(get_authorized_account(env, 1), Some(a.clone()));
            assert_eq!(get_authorized_account(env, 2), Some(b.clone()));

            // Cross-checks must not match
            assert!(!is_authorized_account(env, 1, &b));
            assert!(!is_authorized_account(env, 2, &a));
        });
    }

    #[test]
    fn remove_one_token_does_not_affect_another() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            set_authorized_account(env, 1, &a);
            set_authorized_account(env, 2, &b);

            remove_authorized_account(env, 1);

            assert!(!has_authorized_account(env, 1));
            assert!(has_authorized_account(env, 2));
            assert_eq!(get_authorized_account(env, 2), Some(b));
        });
    }
}
