//! Transfer authorization guard (Issue #1024).
//!
//! Answers the single focused question: **"is this caller permitted to move
//! this token?"** — independently of whether the token is frozen, blacklisted,
//! or the destination is valid. Those concerns live in
//! [`crate::transfer_guard`].
//!
//! # Acceptance criteria
//!
//! | Criterion | Guard function |
//! |-----------|---------------|
//! | Support owner transfers | [`check_owner`] |
//! | Support approved operators (single-token) | [`check_single_token_approval`] |
//! | Support approved operators (all-token) | [`check_operator_for_all`] |
//! | Support admin override | [`check_admin`] |
//! | Reject unauthorized callers | enforced by [`require_transfer_authorization`] |
//! | Integrate with transfer logic | [`require_transfer_authorization`] is the integration point called by [`crate::transfer_guard::check_caller_authorized`] |
//!
//! # Guard priority
//!
//! [`require_transfer_authorization`] tests each claim in this order, short-
//! circuiting on the first match.  If none match the caller is rejected with
//! [`Error::Unauthorized`].
//!
//! 1. Owner — the current `from` address.
//! 2. Single-token approval — an address granted approval for `token_id`
//!    specifically (analogous to ERC-721 `approve`).
//! 3. Operator-for-all — an operator approved for every token owned by `from`
//!    (analogous to ERC-721 `setApprovalForAll`).
//! 4. Admin — the contract-level administrator (emergency override).
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full authorization check — called by transfer_guard:
//! transfer_auth_guard::require_transfer_authorization(&env, &caller, &from, token_id)?;
//!
//! // Individual probes — useful when callers need richer diagnostics:
//! let is_owner    = transfer_auth_guard::check_owner(&caller, &from);
//! let is_approved = transfer_auth_guard::check_single_token_approval(&env, &caller, token_id);
//! let is_operator = transfer_auth_guard::check_operator_for_all(&env, &caller, &from);
//! let is_admin    = transfer_auth_guard::check_admin(&env, &caller);
//! ```

use soroban_sdk::{Address, Env};

use crate::operator_approval;
use crate::owner_storage;
use crate::token_approval;
use crate::types::{Error, TokenId};

// ─── Primary integration point ────────────────────────────────────────────────

/// Assert that `caller` is authorized to transfer `token_id` currently owned
/// by `from`.
///
/// This is the function transfer entry-points (and [`crate::transfer_guard`])
/// must call to enforce authorization.  It demands a Soroban authentication
/// envelope from `caller` via [`Address::require_auth`] before running any
/// identity check, so the host always validates the cryptographic signature.
///
/// A caller is authorized when **any** of the following holds:
///
/// 1. `caller == from` — the token owner.
/// 2. `caller` has been granted a single-token approval for `token_id`.
/// 3. `caller` is an operator approved for **all** tokens of `from`.
/// 4. `caller` is the contract administrator (admin override).
///
/// # Errors
/// - [`Error::Unauthorized`] — none of the four claims hold.
pub fn require_transfer_authorization(
    env: &Env,
    caller: &Address,
    from: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // Demand a signed authorization envelope before any identity check.
    caller.require_auth();

    if check_owner(caller, from)
        || check_single_token_approval(env, caller, token_id)
        || check_operator_for_all(env, caller, from)
        || check_admin(env, caller)
    {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

// ─── Individual authorization probes ─────────────────────────────────────────

/// Return `true` when `caller` is the current owner of the token (`from`).
///
/// Covers acceptance criterion: **support owner transfers**.
///
/// This is a pure address comparison with no storage reads and no auth call;
/// use [`require_transfer_authorization`] when the full guard is needed.
#[inline]
pub fn check_owner(caller: &Address, from: &Address) -> bool {
    caller == from
}

/// Return `true` when `caller` holds a single-token approval for `token_id`.
///
/// Covers acceptance criterion: **support approved operators** (ERC-721
/// `approve` analogue).  Reads the approval record written by
/// [`crate::token_approval::save_approval`].
///
/// Returns `false` if no approval record exists.
pub fn check_single_token_approval(env: &Env, caller: &Address, token_id: TokenId) -> bool {
    token_approval::get_approval(env, token_id)
        .as_ref()
        .map(|approved| approved == caller)
        .unwrap_or(false)
}

/// Return `true` when `caller` is an operator approved to manage all tokens
/// owned by `from`.
///
/// Covers acceptance criterion: **support approved operators** (ERC-721
/// `setApprovalForAll` analogue).  Reads the approval record written by
/// [`crate::operator_approval::save_operator`].
pub fn check_operator_for_all(env: &Env, caller: &Address, from: &Address) -> bool {
    operator_approval::is_operator(env, from, caller)
}

/// Return `true` when `caller` is the contract-level administrator.
///
/// Covers acceptance criterion: **admin emergency override**.  Reads the owner
/// record written by [`crate::owner_storage::save_owner`].
///
/// Returns `false` when the contract has not been initialized (no admin stored
/// yet) rather than panicking, so the caller still receives `Unauthorized`
/// instead of a host trap.
pub fn check_admin(env: &Env, caller: &Address) -> bool {
    owner_storage::get_owner(env)
        .map(|admin| admin == *caller)
        .unwrap_or(false)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_approval;
    use crate::owner_storage;
    use crate::token_approval;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Register the contract and run `f` inside its execution context.
    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    /// Persist a minimal ownership record for `token_id` → `owner`.
    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    // ── check_owner ───────────────────────────────────────────────────────────

    #[test]
    fn check_owner_returns_true_when_caller_is_from() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert!(check_owner(&owner, &owner));
        });
    }

    #[test]
    fn check_owner_returns_false_when_caller_differs_from_from() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            assert!(!check_owner(&stranger, &owner));
        });
    }

    // ── check_single_token_approval ───────────────────────────────────────────

    #[test]
    fn approval_returns_true_when_caller_holds_token_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);

            assert!(check_single_token_approval(env, &approved, 1));
        });
    }

    #[test]
    fn approval_returns_false_when_no_approval_set() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(!check_single_token_approval(env, &caller, 1));
        });
    }

    #[test]
    fn approval_returns_false_for_different_caller() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            let other = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);

            assert!(!check_single_token_approval(env, &other, 1));
        });
    }

    #[test]
    fn approval_returns_false_after_removal() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            token_approval::remove_approval(env, 1);

            assert!(!check_single_token_approval(env, &approved, 1));
        });
    }

    #[test]
    fn approval_is_per_token_and_does_not_bleed_across_tokens() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);
            // Approval only for token 1.
            token_approval::save_approval(env, 1, &approved);

            assert!(check_single_token_approval(env, &approved, 1));
            assert!(!check_single_token_approval(env, &approved, 2));
        });
    }

    // ── check_operator_for_all ────────────────────────────────────────────────

    #[test]
    fn operator_for_all_returns_true_when_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(check_operator_for_all(env, &operator, &owner));
        });
    }

    #[test]
    fn operator_for_all_returns_false_when_not_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);

            assert!(!check_operator_for_all(env, &operator, &owner));
        });
    }

    #[test]
    fn operator_for_all_returns_false_after_revocation() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            operator_approval::save_operator(env, &owner, &operator);
            operator_approval::remove_operator(env, &owner, &operator);

            assert!(!check_operator_for_all(env, &operator, &owner));
        });
    }

    #[test]
    fn operator_approval_is_owner_scoped_and_does_not_bleed() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let operator = Address::generate(env);
            // Approved only for owner_a.
            operator_approval::save_operator(env, &owner_a, &operator);

            assert!(check_operator_for_all(env, &operator, &owner_a));
            assert!(!check_operator_for_all(env, &operator, &owner_b));
        });
    }

    // ── check_admin ───────────────────────────────────────────────────────────

    #[test]
    fn admin_check_returns_true_for_stored_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            owner_storage::save_owner(env, &admin);

            assert!(check_admin(env, &admin));
        });
    }

    #[test]
    fn admin_check_returns_false_for_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            owner_storage::save_owner(env, &admin);

            assert!(!check_admin(env, &other));
        });
    }

    #[test]
    fn admin_check_returns_false_when_no_admin_stored() {
        with_contract(|env| {
            let caller = Address::generate(env);
            // No admin has been stored — must return false, not panic.
            assert!(!check_admin(env, &caller));
        });
    }

    #[test]
    fn admin_check_reflects_updated_admin() {
        with_contract(|env| {
            let old_admin = Address::generate(env);
            let new_admin = Address::generate(env);
            owner_storage::save_owner(env, &old_admin);
            owner_storage::save_owner(env, &new_admin);

            assert!(!check_admin(env, &old_admin));
            assert!(check_admin(env, &new_admin));
        });
    }

    // ── require_transfer_authorization — owner ────────────────────────────────

    #[test]
    fn owner_is_authorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            assert!(require_transfer_authorization(env, &owner, &owner, 1).is_ok());
        });
    }

    #[test]
    fn require_auth_is_invoked_for_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            require_transfer_authorization(env, &owner, &owner, 1).unwrap();

            let auths = env.auths();
            assert!(!auths.is_empty());
            // First auth envelope must belong to the owner.
            assert_eq!(unsafe { auths.get_unchecked(0).0.clone() }, owner);
        });
    }

    // ── require_transfer_authorization — single-token approval ───────────────

    #[test]
    fn approved_address_is_authorized_for_specific_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);

            assert!(require_transfer_authorization(env, &approved, &owner, 1).is_ok());
        });
    }

    #[test]
    fn approval_for_token_1_does_not_authorize_token_2() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);
            token_approval::save_approval(env, 1, &approved);

            // Token 2 has no approval — must be rejected.
            assert_eq!(
                require_transfer_authorization(env, &approved, &owner, 2),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn removed_approval_no_longer_authorizes() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            token_approval::remove_approval(env, 1);

            assert_eq!(
                require_transfer_authorization(env, &approved, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── require_transfer_authorization — operator-for-all ────────────────────

    #[test]
    fn operator_for_all_is_authorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(require_transfer_authorization(env, &operator, &owner, 1).is_ok());
        });
    }

    #[test]
    fn operator_for_all_covers_any_token_of_that_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 10, &owner);
            setup_token(env, 20, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(require_transfer_authorization(env, &operator, &owner, 10).is_ok());
            assert!(require_transfer_authorization(env, &operator, &owner, 20).is_ok());
        });
    }

    #[test]
    fn revoked_operator_is_unauthorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);
            operator_approval::remove_operator(env, &owner, &operator);

            assert_eq!(
                require_transfer_authorization(env, &operator, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn operator_approved_for_different_owner_is_unauthorized() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner_a);
            // Approved for owner_b, not owner_a.
            operator_approval::save_operator(env, &owner_b, &operator);

            assert_eq!(
                require_transfer_authorization(env, &operator, &owner_a, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── require_transfer_authorization — admin override ───────────────────────

    #[test]
    fn admin_is_authorized_to_transfer_any_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            setup_token(env, 1, &owner);
            owner_storage::save_owner(env, &admin);

            assert!(require_transfer_authorization(env, &admin, &owner, 1).is_ok());
        });
    }

    // ── require_transfer_authorization — unauthorized rejection ───────────────

    #[test]
    fn stranger_is_unauthorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);

            assert_eq!(
                require_transfer_authorization(env, &stranger, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn address_with_no_approval_and_no_role_is_unauthorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let nobody = Address::generate(env);
            setup_token(env, 1, &owner);

            // Explicitly set an admin that is NOT `nobody`.
            let admin = Address::generate(env);
            owner_storage::save_owner(env, &admin);

            assert_eq!(
                require_transfer_authorization(env, &nobody, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── Guard priority (short-circuit ordering) ───────────────────────────────

    #[test]
    fn owner_is_accepted_even_if_approval_also_exists() {
        // Verifies correct short-circuit: owner match wins before approval check.
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            // Also set an approval for some other address.
            let approved = Address::generate(env);
            token_approval::save_approval(env, 1, &approved);

            assert!(require_transfer_authorization(env, &owner, &owner, 1).is_ok());
        });
    }

    #[test]
    fn approval_is_accepted_even_when_operator_approval_also_exists() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            // Approved also has operator-for-all — should still pass.
            operator_approval::save_operator(env, &owner, &approved);

            assert!(require_transfer_authorization(env, &approved, &owner, 1).is_ok());
        });
    }
}
