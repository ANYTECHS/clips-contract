//! Creator authorization guard for creator-owned NFT operations (issue #1249).
//!
//! Restricts creator-specific operations to:
//! 1. The **registered creator** of the token, or
//! 2. An **explicitly authorized delegate** set via
//!    [`crate::creator_auth_storage`].
//!
//! # Design
//!
//! | Function | Authorized callers |
//! |----------|--------------------|
//! | [`require_creator`] | Registered creator only |
//! | [`require_creator_or_authorized`] | Registered creator **or** authorized delegate |
//! | [`is_creator_or_authorized`] | Non-panicking probe — same as above but returns `bool` |
//!
//! All entry points call `caller.require_auth()` on the matched identity so
//! the Soroban host enforces the cryptographic signature before the function
//! returns `Ok(())`.
//!
//! # Errors (in evaluation order)
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::TokenNotFound`] | No creator record exists for `token_id`. |
//! | [`Error::Unauthorized`]  | Caller is neither the creator nor the authorized delegate. |

use soroban_sdk::{Address, Env};

use crate::creator_auth_storage;
use crate::creator_storage;
use crate::types::{Error, TokenId};

// ─── Primary entry points ─────────────────────────────────────────────────────

/// Verify that `caller` is the registered creator of `token_id`.
///
/// The creator record is read from persistent storage.  A missing record is
/// propagated as [`Error::TokenNotFound`], while a different caller is
/// rejected as [`Error::Unauthorized`].
///
/// `require_auth` is called on the creator address when the check passes,
/// so the Soroban host enforces the cryptographic signature.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no creator record exists for `token_id`.
/// - [`Error::Unauthorized`]  — `caller` is not the registered creator.
pub fn require_creator(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let creator = creator_storage::get_creator(env, token_id)?;
    if creator != *caller {
        return Err(Error::Unauthorized);
    }

    caller.require_auth();
    Ok(())
}

/// Verify that `caller` is either the registered creator **or** the explicitly
/// authorized delegate for `token_id`.
///
/// Authorization order (first match wins):
/// 1. Registered creator ([`crate::creator_storage`]).
/// 2. Explicitly authorized delegate ([`crate::creator_auth_storage`]).
///
/// `require_auth` is called on the matched identity so the Soroban host
/// enforces the cryptographic signature before the function returns `Ok(())`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no creator record exists for `token_id`.
/// - [`Error::Unauthorized`]  — `caller` is neither the creator nor the
///                              authorized delegate.
pub fn require_creator_or_authorized(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // Retrieve the creator — propagates TokenNotFound when no record exists.
    let creator = creator_storage::get_creator(env, token_id)?;

    // 1. Caller is the registered creator.
    if *caller == creator {
        caller.require_auth();
        return Ok(());
    }

    // 2. Caller is the explicitly authorized delegate.
    if creator_auth_storage::is_authorized_account(env, token_id, caller) {
        caller.require_auth();
        return Ok(());
    }

    Err(Error::Unauthorized)
}

// ─── Non-authoritative probe ──────────────────────────────────────────────────

/// Return `true` when `caller` is either the registered creator **or** the
/// authorized delegate for `token_id`, without consuming an auth envelope.
///
/// This is a read-only probe suitable for conditional branching or building UI
/// state.  It does **not** call `require_auth` and does **not** enforce any
/// cryptographic check.
///
/// Returns `false` (not an error) when `token_id` has no creator record —
/// treat a missing record as "no authorization".  Use
/// [`require_creator_or_authorized`] when you need a hard error on missing
/// tokens.
pub fn is_creator_or_authorized(env: &Env, caller: &Address, token_id: TokenId) -> bool {
    let Ok(creator) = creator_storage::get_creator(env, token_id) else {
        return false;
    };

    if *caller == creator {
        return true;
    }

    creator_auth_storage::is_authorized_account(env, token_id, caller)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::creator_auth_storage;
    use crate::creator_storage;
    use soroban_sdk::testutils::Address as _;

    // ── require_creator ───────────────────────────────────────────────────────

    #[test]
    fn registered_creator_passes_guard() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert!(require_creator(&env, &creator, 1).is_ok());
    }

    #[test]
    fn different_caller_is_rejected_by_require_creator() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let caller = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert_eq!(require_creator(&env, &caller, 1), Err(Error::Unauthorized));
    }

    #[test]
    fn missing_creator_record_is_rejected_by_require_creator() {
        let env = soroban_sdk::Env::default();
        let caller = Address::generate(&env);

        assert_eq!(
            require_creator(&env, &caller, 1),
            Err(Error::TokenNotFound)
        );
    }

    /// Authorized delegate must NOT pass require_creator (creator-only check).
    #[test]
    fn authorized_delegate_is_rejected_by_require_creator() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);

        assert_eq!(
            require_creator(&env, &delegate, 1),
            Err(Error::Unauthorized)
        );
    }

    // ── require_creator_or_authorized ─────────────────────────────────────────

    #[test]
    fn creator_passes_creator_or_authorized() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert!(require_creator_or_authorized(&env, &creator, 1).is_ok());
    }

    #[test]
    fn authorized_delegate_passes_creator_or_authorized() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);

        assert!(require_creator_or_authorized(&env, &delegate, 1).is_ok());
    }

    #[test]
    fn unauthorized_third_party_is_rejected() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        let stranger = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);

        assert_eq!(
            require_creator_or_authorized(&env, &stranger, 1),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn missing_token_returns_token_not_found() {
        let env = soroban_sdk::Env::default();
        let caller = Address::generate(&env);

        assert_eq!(
            require_creator_or_authorized(&env, &caller, 99),
            Err(Error::TokenNotFound)
        );
    }

    /// After revoking the delegate, the former delegate must be rejected.
    #[test]
    fn revoked_delegate_is_rejected() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);
        creator_auth_storage::remove_authorized_account(&env, 1);

        assert_eq!(
            require_creator_or_authorized(&env, &delegate, 1),
            Err(Error::Unauthorized)
        );
    }

    /// After replacing the delegate, the old delegate must be rejected and the
    /// new delegate must be accepted.
    #[test]
    fn replaced_delegate_old_rejected_new_accepted() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let old_delegate = Address::generate(&env);
        let new_delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &old_delegate);
        creator_auth_storage::set_authorized_account(&env, 1, &new_delegate);

        assert_eq!(
            require_creator_or_authorized(&env, &old_delegate, 1),
            Err(Error::Unauthorized)
        );
        assert!(require_creator_or_authorized(&env, &new_delegate, 1).is_ok());
    }

    /// Guard is per-token: authorization for token 1 must not bleed to token 2.
    #[test]
    fn authorization_is_isolated_per_token() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_storage::set_creator(&env, 2, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);
        // Delegate is only set for token 1, NOT token 2.

        assert!(require_creator_or_authorized(&env, &delegate, 1).is_ok());
        assert_eq!(
            require_creator_or_authorized(&env, &delegate, 2),
            Err(Error::Unauthorized)
        );
    }

    // ── is_creator_or_authorized (probe) ──────────────────────────────────────

    #[test]
    fn probe_returns_true_for_creator() {
        let env = soroban_sdk::Env::default();
        let creator = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert!(is_creator_or_authorized(&env, &creator, 1));
    }

    #[test]
    fn probe_returns_true_for_authorized_delegate() {
        let env = soroban_sdk::Env::default();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);

        assert!(is_creator_or_authorized(&env, &delegate, 1));
    }

    #[test]
    fn probe_returns_false_for_stranger() {
        let env = soroban_sdk::Env::default();
        let creator = Address::generate(&env);
        let stranger = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);

        assert!(!is_creator_or_authorized(&env, &stranger, 1));
    }

    #[test]
    fn probe_returns_false_for_missing_token() {
        let env = soroban_sdk::Env::default();
        let caller = Address::generate(&env);

        assert!(!is_creator_or_authorized(&env, &caller, 999));
    }

    #[test]
    fn probe_returns_false_after_delegate_revoked() {
        let env = soroban_sdk::Env::default();
        let creator = Address::generate(&env);
        let delegate = Address::generate(&env);
        creator_storage::set_creator(&env, 1, &creator);
        creator_auth_storage::set_authorized_account(&env, 1, &delegate);
        creator_auth_storage::remove_authorized_account(&env, 1);

        assert!(!is_creator_or_authorized(&env, &delegate, 1));
    }
}
