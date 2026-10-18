//! Operator approval guard (Issue #1092).
//!
//! Implements a guard for validating whether an account has been approved to
//! operate on behalf of an NFT owner.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Retrieve approval information | [`get_operator_approval`] |
//! | Validate operator authorization | [`check_operator_approved`] |
//! | Reject unauthorized operators | [`require_operator_approval`] |
//! | Add operator guard tests | Comprehensive test module |
//!
//! # Authorization model
//!
//! An operator may be authorized in two ways:
//!
//! 1. **Single-token approval** — approved for a specific `token_id` only
//!    (analogous to ERC-721 `approve`).
//! 2. **Operator-for-all** — approved to operate on **every** token owned by
//!    `owner` (analogous to ERC-721 `setApprovalForAll`).
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full guard with auth requirement
//! operator_approval_guard::require_operator_approval(&env, &caller, &owner, token_id)?;
//!
//! // Check approval without auth
//! if operator_approval_guard::check_operator_approved(&env, &caller, &owner, token_id) {
//!     // caller is approved to operate on the token
//! }
//!
//! // Retrieve approval info
//! let approval_type = operator_approval_guard::get_operator_approval(&env, &caller, &owner, token_id);
//! ```

use soroban_sdk::{Address, Env};

use crate::operator_approval;
use crate::token_approval;
use crate::types::TokenId;

// ─── Approval type enum ───────────────────────────────────────────────────────

/// Describes the type of operator approval granted.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ApprovalType {
    /// Operator is approved for a specific token only.
    SingleToken,
    /// Operator is approved for all tokens owned by the owner.
    OperatorForAll,
}

// ─── Primary integration point ────────────────────────────────────────────────

/// Require that `caller` is an approved operator for `token_id` owned by `owner`.
///
/// Verifies operator authorization and demands an authorization signature from
/// the caller via [`Address::require_auth`]. If the caller is not an approved
/// operator, returns [`crate::types::Error::Unauthorized`].
///
/// A caller is an approved operator if **any** of the following holds:
///
/// 1. The caller holds a single-token approval for `token_id`.
/// 2. The caller is an operator approved for **all** tokens of `owner`.
///
/// This is the authoritative guard for operator operations. Use it whenever you
/// need to verify both operator status **and** authorize a sensitive operation.
///
/// # Errors
/// - [`crate::types::Error::Unauthorized`] — caller is not an approved operator.
///
/// # Example
///
/// ```rust,ignore
/// operator_approval_guard::require_operator_approval(&env, &caller, &owner, token_id)?;
/// // If Ok, caller is authorized as an operator; safe to proceed
/// ```
pub fn require_operator_approval(
    env: &Env,
    caller: &Address,
    owner: &Address,
    token_id: TokenId,
) -> Result<(), crate::types::Error> {
    // Demand a signed authorization envelope from the caller
    caller.require_auth();

    if check_operator_approved(env, caller, owner, token_id) {
        return Ok(());
    }

    Err(crate::types::Error::Unauthorized)
}

// ─── Individual authorization probes ─────────────────────────────────────────

/// Return `true` when `caller` is an approved operator for `token_id`.
///
/// Performs no authorization call and no error handling—purely a read-only
/// check. Use this when you already have the required authorization or need
/// to make a non-authoritative probe.
///
/// A caller is an approved operator if **any** of the following holds:
///
/// 1. The caller holds a single-token approval for `token_id`.
/// 2. The caller is an operator approved for **all** tokens of `owner`.
///
/// Returns `false` if neither approval exists.
///
/// # Example
///
/// ```rust,ignore
/// if operator_approval_guard::check_operator_approved(&env, &caller, &owner, token_id) {
///     // caller is an approved operator; safe to proceed
/// }
/// ```
pub fn check_operator_approved(
    env: &Env,
    caller: &Address,
    owner: &Address,
    token_id: TokenId,
) -> bool {
    // Priority 1: Check single-token approval
    if token_approval::get_approval(env, token_id)
        .as_ref()
        .map(|approved| approved == caller)
        .unwrap_or(false)
    {
        return true;
    }

    // Priority 2: Check operator-for-all approval
    if operator_approval::is_operator(env, owner, caller) {
        return true;
    }

    false
}

/// Retrieve the type of operator approval held by `caller` for `token_id`.
///
/// Returns `Some(ApprovalType)` if the caller holds any approval, or `None`
/// if the caller is not an approved operator.
///
/// When multiple approval types exist, returns the first match in priority order:
/// 1. Single-token approval
/// 2. Operator-for-all approval
///
/// # Example
///
/// ```rust,ignore
/// match operator_approval_guard::get_operator_approval(&env, &caller, &owner, token_id) {
///     Some(ApprovalType::SingleToken) => { /* token-specific */ },
///     Some(ApprovalType::OperatorForAll) => { /* all tokens */ },
///     None => { /* not approved */ },
/// }
/// ```
pub fn get_operator_approval(
    env: &Env,
    caller: &Address,
    owner: &Address,
    token_id: TokenId,
) -> Option<ApprovalType> {
    // Priority 1: Single-token approval
    if token_approval::get_approval(env, token_id)
        .as_ref()
        .map(|approved| approved == caller)
        .unwrap_or(false)
    {
        return Some(ApprovalType::SingleToken);
    }

    // Priority 2: Operator-for-all approval
    if operator_approval::is_operator(env, owner, caller) {
        return Some(ApprovalType::OperatorForAll);
    }

    None
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_approval;
    use crate::token_approval;
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

    // ── require_operator_approval ──────────────────────────────────────────

    #[test]
    fn require_operator_approval_accepts_single_token_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            token_approval::save_approval(env, token_id, &operator);

            assert!(require_operator_approval(env, &operator, &owner, token_id).is_ok());
        });
    }

    #[test]
    fn require_operator_approval_accepts_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(require_operator_approval(env, &operator, &owner, token_id).is_ok());
        });
    }

    #[test]
    fn require_operator_approval_rejects_non_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let other = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert_eq!(
                require_operator_approval(env, &other, &owner, token_id),
                Err(crate::types::Error::Unauthorized)
            );
        });
    }

    // ── check_operator_approved ────────────────────────────────────────────

    #[test]
    fn check_operator_approved_returns_true_for_single_token_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            token_approval::save_approval(env, token_id, &operator);

            assert!(check_operator_approved(env, &operator, &owner, token_id));
        });
    }

    #[test]
    fn check_operator_approved_returns_true_for_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(check_operator_approved(env, &operator, &owner, token_id));
        });
    }

    #[test]
    fn check_operator_approved_returns_false_for_non_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let other = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(!check_operator_approved(env, &other, &owner, token_id));
        });
    }

    // ── get_operator_approval ──────────────────────────────────────────────

    #[test]
    fn get_operator_approval_returns_single_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            token_approval::save_approval(env, token_id, &operator);

            assert_eq!(
                get_operator_approval(env, &operator, &owner, token_id),
                Some(ApprovalType::SingleToken)
            );
        });
    }

    #[test]
    fn get_operator_approval_returns_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert_eq!(
                get_operator_approval(env, &operator, &owner, token_id),
                Some(ApprovalType::OperatorForAll)
            );
        });
    }

    #[test]
    fn get_operator_approval_returns_none_for_non_approved() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert_eq!(
                get_operator_approval(env, &operator, &owner, token_id),
                None
            );
        });
    }

    // ── Priority order verification ────────────────────────────────────────

    #[test]
    fn get_operator_approval_prefers_single_token_over_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            // Both approvals exist
            token_approval::save_approval(env, token_id, &operator);
            operator_approval::save_operator(env, &owner, &operator);

            // Should return single-token (higher priority)
            assert_eq!(
                get_operator_approval(env, &operator, &owner, token_id),
                Some(ApprovalType::SingleToken)
            );
        });
    }

    // ── Multiple token independence ────────────────────────────────────────

    #[test]
    fn check_operator_approved_is_token_specific_for_single_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_1 = 1;
            let token_2 = 2;

            setup_token(env, token_1, &owner);
            setup_token(env, token_2, &owner);

            // Approval for token_1 only
            token_approval::save_approval(env, token_1, &operator);

            // Should be approved for token_1
            assert!(check_operator_approved(env, &operator, &owner, token_1));
            // Should not be approved for token_2
            assert!(!check_operator_approved(env, &operator, &owner, token_2));
        });
    }

    #[test]
    fn check_operator_approved_is_universal_for_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_1 = 1;
            let token_2 = 2;

            setup_token(env, token_1, &owner);
            setup_token(env, token_2, &owner);

            // Operator-for-all approval
            operator_approval::save_operator(env, &owner, &operator);

            // Should be approved for both tokens
            assert!(check_operator_approved(env, &operator, &owner, token_1));
            assert!(check_operator_approved(env, &operator, &owner, token_2));
        });
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn operator_approval_is_deterministic_across_calls() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            token_approval::save_approval(env, token_id, &operator);

            // Multiple calls should return consistent results
            assert!(check_operator_approved(env, &operator, &owner, token_id));
            assert!(check_operator_approved(env, &operator, &owner, token_id));
            assert!(check_operator_approved(env, &operator, &owner, token_id));
        });
    }

    #[test]
    fn multiple_operators_are_tracked_independently() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator_1 = Address::generate(env);
            let operator_2 = Address::generate(env);
            let token_id = 1;

            setup_token(env, token_id, &owner);
            operator_approval::save_operator(env, &owner, &operator_1);
            operator_approval::save_operator(env, &owner, &operator_2);

            // Both should be approved
            assert!(check_operator_approved(env, &operator_1, &owner, token_id));
            assert!(check_operator_approved(env, &operator_2, &owner, token_id));
        });
    }
}
