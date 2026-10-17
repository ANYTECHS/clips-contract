//! Token Approval Authorization Guard (Issue #1259).
//!
//! Enforces token approval authorization rules ensuring that token approvals
//! can only be granted or revoked by accounts permitted under standard NFT
//! approval rules (ERC-721 / Soroban NFT standard).
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Verify the NFT owner or permitted authority | [`check_approval_authority`], [`check_owner`], [`check_operator_for_all`], [`check_admin`] |
//! | Validate the target operator | [`validate_target_operator`], [`check_target_operator_valid`] |
//! | Reject unauthorized approval changes | Enforced by [`require_token_approval_auth`], [`require_token_approval_revoke_auth`], [`require_operator_for_all_auth`] |
//! | Add approval authorization tests | Comprehensive unit test suite in `#[cfg(test)]` block |
//!
//! # Authorization model
//!
//! Under standard NFT approval rules:
//!
//! 1. **NFT Owner** — The current recorded owner of the token always holds full
//!    authority to grant or revoke single-token approvals and operator-for-all
//!    permissions.
//! 2. **Operator for all** — An account holding operator-for-all permission
//!    for the token owner (via `set_approval_for_all`) is a permitted authority
//!    and may grant or revoke single-token approvals on behalf of the owner.
//! 3. **Admin override** — The contract-level administrator may act as an
//!    emergency permitted authority.
//! 4. **Single-token approved account** — Holds permission to *transfer* only;
//!    does **not** hold authority to grant approval to other operators.
//! 5. **Unauthorized accounts** — Any third-party without ownership or operator
//!    status is strictly rejected with [`Error::Unauthorized`].
//!
//! # Target operator validation rules
//!
//! When granting an approval to a target operator:
//! - Operator cannot be the current token owner ([`Error::InvalidRecipient`]).
//! - Operator cannot be the smart contract address itself ([`Error::InvalidRecipient`]).
//! - Operator cannot be blacklisted ([`Error::Unauthorized`]).

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::frozen_token;
use crate::operator_approval;
use crate::owner_storage;
use crate::token_approval;
use crate::token_lifecycle;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Authority checks ────────────────────────────────────────────────────────

/// Return `true` if `caller` is the recorded owner of the token.
#[inline]
pub fn check_owner(caller: &Address, owner: &Address) -> bool {
    caller == owner
}

/// Return `true` if `caller` is an approved operator for all tokens of `owner`.
#[inline]
pub fn check_operator_for_all(env: &Env, caller: &Address, owner: &Address) -> bool {
    operator_approval::is_operator(env, owner, caller)
}

/// Return `true` if `caller` is the contract administrator.
pub fn check_admin(env: &Env, caller: &Address) -> bool {
    owner_storage::get_owner(env)
        .map(|admin| admin == *caller)
        .unwrap_or(false)
        || crate::administrator_storage::is_admin(env, caller)
}

/// Verify whether `caller` is the NFT owner or a permitted authority (operator-for-all or admin).
///
/// A caller has approval authority if **any** of the following holds:
/// 1. `caller == owner` (the token owner).
/// 2. `caller` is an operator approved for all tokens of `owner`.
/// 3. `caller` is the contract administrator.
///
/// Note: A single-token approved operator does **not** have authority to grant or
/// revoke approvals for other accounts.
pub fn check_approval_authority(env: &Env, caller: &Address, owner: &Address) -> bool {
    if check_owner(caller, owner) {
        return true;
    }
    if check_operator_for_all(env, caller, owner) {
        return true;
    }
    if check_admin(env, caller) {
        return true;
    }
    false
}

// ─── Target operator validation ──────────────────────────────────────────────

/// Validate the target operator receiving the approval.
///
/// # Validation rules
/// - `operator` cannot be the token `owner` (approving oneself is invalid; [`Error::InvalidRecipient`]).
/// - `operator` cannot be the smart contract itself ([`Error::InvalidRecipient`]).
/// - `operator` cannot be blacklisted ([`Error::Unauthorized`]).
///
/// # Errors
/// - [`Error::InvalidRecipient`] if `operator == owner` or `operator == current_contract_address`.
/// - [`Error::Unauthorized`] if `operator` is blacklisted.
pub fn validate_target_operator(
    env: &Env,
    owner: &Address,
    operator: &Address,
) -> Result<(), Error> {
    if operator == owner {
        return Err(Error::InvalidRecipient);
    }
    if operator == &env.current_contract_address() {
        return Err(Error::InvalidRecipient);
    }
    if blacklist::is_blacklisted(env, operator) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Non-failing probe checking whether `operator` is valid to receive an approval.
pub fn check_target_operator_valid(env: &Env, owner: &Address, operator: &Address) -> bool {
    validate_target_operator(env, owner, operator).is_ok()
}

// ─── Primary guard entry points ──────────────────────────────────────────────

/// Enforce authorization to grant a token approval.
///
/// Verifies that:
/// 1. `caller` signs the invocation via [`Address::require_auth`].
/// 2. The referenced `token_id` exists ([`Error::TokenNotFound`]).
/// 3. The token is active and not burned ([`Error::AlreadyBurned`]).
/// 4. The token is not frozen / soulbound ([`Error::Unauthorized`]).
/// 5. `caller` is the NFT owner or a permitted authority ([`Error::Unauthorized`]).
/// 6. The target `operator` is valid according to approval rules ([`validate_target_operator`]).
///
/// # Returns
/// Returns `Ok(owner)` with the current token owner on success.
///
/// # Errors
/// - [`Error::TokenNotFound`] if no owner record exists for `token_id`.
/// - [`Error::AlreadyBurned`] if the token was burned.
/// - [`Error::Unauthorized`] if the token is frozen, `caller` is unauthorized, or `operator` is blacklisted.
/// - [`Error::InvalidRecipient`] if `operator` is the owner or current contract address.
pub fn require_token_approval_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
    operator: &Address,
) -> Result<Address, Error> {
    caller.require_auth();

    // 1. Verify token existence and retrieve owner
    let owner = token_owner_storage::get_owner(env, token_id)?;

    // 2. Validate token lifecycle state
    if token_lifecycle::is_burned(env, token_id) {
        return Err(Error::AlreadyBurned);
    }
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }

    // 3. Verify caller authority (owner or permitted authority)
    if !check_approval_authority(env, caller, &owner) {
        return Err(Error::Unauthorized);
    }

    // 4. Validate target operator
    validate_target_operator(env, &owner, operator)?;

    Ok(owner)
}

/// Enforce authorization to revoke a token approval.
///
/// Verifies that:
/// 1. `caller` signs the invocation via [`Address::require_auth`].
/// 2. The referenced `token_id` exists ([`Error::TokenNotFound`]).
/// 3. The token is active and not burned ([`Error::AlreadyBurned`]).
/// 4. `caller` is the NFT owner or a permitted authority ([`Error::Unauthorized`]).
///
/// # Returns
/// Returns `Ok(owner)` with the current token owner on success.
///
/// # Errors
/// - [`Error::TokenNotFound`] if no owner record exists for `token_id`.
/// - [`Error::AlreadyBurned`] if the token was burned.
/// - [`Error::Unauthorized`] if `caller` lacks approval authority.
pub fn require_token_approval_revoke_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<Address, Error> {
    caller.require_auth();

    // 1. Verify token existence and retrieve owner
    let owner = token_owner_storage::get_owner(env, token_id)?;

    // 2. Validate token lifecycle state
    if token_lifecycle::is_burned(env, token_id) {
        return Err(Error::AlreadyBurned);
    }

    // 3. Verify caller authority (owner or permitted authority)
    if !check_approval_authority(env, caller, &owner) {
        return Err(Error::Unauthorized);
    }

    Ok(owner)
}

/// Enforce authorization to grant or revoke operator-for-all approval.
///
/// Only the token owner (or contract admin override) may grant or revoke
/// operator-for-all permissions over an account.
///
/// # Errors
/// - [`Error::Unauthorized`] if `caller` is not `owner` or contract admin.
/// - [`Error::InvalidRecipient`] if `operator == owner` or `operator == current_contract`.
pub fn require_operator_for_all_auth(
    env: &Env,
    caller: &Address,
    owner: &Address,
    operator: &Address,
) -> Result<(), Error> {
    caller.require_auth();

    if caller != owner && !check_admin(env, caller) {
        return Err(Error::Unauthorized);
    }

    validate_target_operator(env, owner, operator)?;
    Ok(())
}

// ─── High-level composite helpers ────────────────────────────────────────────

/// Validate authorization and persist single-token approval.
pub fn authorize_and_grant_approval(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
    operator: &Address,
) -> Result<(), Error> {
    require_token_approval_auth(env, caller, token_id, operator)?;
    token_approval::save_approval_checked(env, token_id, operator)
}

/// Validate authorization and revoke single-token approval.
pub fn authorize_and_revoke_approval(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<Address, Error> {
    let owner = require_token_approval_revoke_auth(env, caller, token_id)?;
    token_approval::revoke_approval_checked(env, &owner, token_id)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blacklist;
    use crate::frozen_token;
    use crate::operator_approval;
    use crate::owner_storage;
    use crate::token_approval;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

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

    // ── Acceptance Criterion 1: Verify NFT owner or permitted authority ──────────

    #[test]
    fn owner_has_approval_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert!(check_owner(&owner, &owner));
            assert!(check_approval_authority(env, &owner, &owner));
        });
    }

    #[test]
    fn operator_for_all_has_approval_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);

            operator_approval::save_operator(env, &owner, &operator);

            assert!(check_operator_for_all(env, &operator, &owner));
            assert!(check_approval_authority(env, &operator, &owner));
        });
    }

    #[test]
    fn admin_has_approval_authority() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);

            owner_storage::save_owner(env, &admin);

            assert!(check_admin(env, &admin));
            assert!(check_approval_authority(env, &admin, &owner));
        });
    }

    #[test]
    fn unauthorized_caller_lacks_approval_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);

            assert!(!check_owner(&stranger, &owner));
            assert!(!check_operator_for_all(env, &stranger, &owner));
            assert!(!check_admin(env, &stranger));
            assert!(!check_approval_authority(env, &stranger, &owner));
        });
    }

    #[test]
    fn single_token_approved_address_lacks_approval_authority() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved_for_one = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            token_approval::save_approval(env, token_id, &approved_for_one);

            // Single token approved is NOT owner, NOT operator for all, NOT admin
            assert!(!check_approval_authority(env, &approved_for_one, &owner));
        });
    }

    // ── Acceptance Criterion 2: Validate the target operator ─────────────────────

    #[test]
    fn target_operator_valid_when_distinct_and_not_blacklisted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);

            assert!(validate_target_operator(env, &owner, &operator).is_ok());
            assert!(check_target_operator_valid(env, &owner, &operator));
        });
    }

    #[test]
    fn target_operator_cannot_be_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);

            assert_eq!(
                validate_target_operator(env, &owner, &owner),
                Err(Error::InvalidRecipient)
            );
            assert!(!check_target_operator_valid(env, &owner, &owner));
        });
    }

    #[test]
    fn target_operator_cannot_be_contract_address() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let contract_addr = env.current_contract_address();

            assert_eq!(
                validate_target_operator(env, &owner, &contract_addr),
                Err(Error::InvalidRecipient)
            );
            assert!(!check_target_operator_valid(env, &owner, &contract_addr));
        });
    }

    #[test]
    fn target_operator_cannot_be_blacklisted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);

            blacklist::add_wallet(env, &operator);

            assert_eq!(
                validate_target_operator(env, &owner, &operator),
                Err(Error::Unauthorized)
            );
            assert!(!check_target_operator_valid(env, &owner, &operator));
        });
    }

    // ── Acceptance Criterion 3: Reject unauthorized approval changes ─────────────

    #[test]
    fn owner_can_authorize_token_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            let res = require_token_approval_auth(env, &owner, token_id, &operator);
            assert_eq!(res, Ok(owner));
        });
    }

    #[test]
    fn operator_for_all_can_authorize_token_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator_for_all = Address::generate(env);
            let target_operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            operator_approval::save_operator(env, &owner, &operator_for_all);

            let res = require_token_approval_auth(env, &operator_for_all, token_id, &target_operator);
            assert_eq!(res, Ok(owner));
        });
    }

    #[test]
    fn admin_can_authorize_token_approval() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            let target_operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            owner_storage::save_owner(env, &admin);

            let res = require_token_approval_auth(env, &admin, token_id, &target_operator);
            assert_eq!(res, Ok(owner));
        });
    }

    #[test]
    fn unauthorized_caller_rejected_from_granting_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            let target_operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert_eq!(
                require_token_approval_auth(env, &stranger, token_id, &target_operator),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn single_token_approved_account_rejected_from_granting_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved_delegate = Address::generate(env);
            let third_party = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            token_approval::save_approval(env, token_id, &approved_delegate);

            // Single token delegate must not be allowed to grant approval to someone else
            assert_eq!(
                require_token_approval_auth(env, &approved_delegate, token_id, &third_party),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn granting_approval_for_nonexistent_token_rejected() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let operator = Address::generate(env);

            assert_eq!(
                require_token_approval_auth(env, &caller, 999, &operator),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn granting_approval_for_frozen_token_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            frozen_token::freeze_token(env, token_id);

            assert_eq!(
                require_token_approval_auth(env, &owner, token_id, &operator),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn granting_approval_with_owner_as_operator_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert_eq!(
                require_token_approval_auth(env, &owner, token_id, &owner),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn granting_approval_with_blacklisted_operator_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            blacklist::add_wallet(env, &operator);

            assert_eq!(
                require_token_approval_auth(env, &owner, token_id, &operator),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── Revoke authorization tests ───────────────────────────────────────────────

    #[test]
    fn owner_can_authorize_revoking_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            let res = require_token_approval_revoke_auth(env, &owner, token_id);
            assert_eq!(res, Ok(owner));
        });
    }

    #[test]
    fn operator_for_all_can_authorize_revoking_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            operator_approval::save_operator(env, &owner, &operator);

            let res = require_token_approval_revoke_auth(env, &operator, token_id);
            assert_eq!(res, Ok(owner));
        });
    }

    #[test]
    fn admin_can_authorize_revoking_approval() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            owner_storage::save_owner(env, &admin);

            let res = require_token_approval_revoke_auth(env, &admin, token_id);
            assert_eq!(res, Ok(owner));
        });
    }

    #[test]
    fn unauthorized_caller_rejected_from_revoking_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert_eq!(
                require_token_approval_revoke_auth(env, &stranger, token_id),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn revoking_approval_for_nonexistent_token_rejected() {
        with_contract(|env| {
            let caller = Address::generate(env);

            assert_eq!(
                require_token_approval_revoke_auth(env, &caller, 999),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── Operator for all authorization tests ────────────────────────────────────

    #[test]
    fn owner_can_authorize_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);

            assert!(require_operator_for_all_auth(env, &owner, &owner, &operator).is_ok());
        });
    }

    #[test]
    fn stranger_cannot_authorize_operator_for_all() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            let operator = Address::generate(env);

            assert_eq!(
                require_operator_for_all_auth(env, &stranger, &owner, &operator),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn operator_for_all_rejects_owner_as_operator() {
        with_contract(|env| {
            let owner = Address::generate(env);

            assert_eq!(
                require_operator_for_all_auth(env, &owner, &owner, &owner),
                Err(Error::InvalidRecipient)
            );
        });
    }

    // ── High-level composite helper tests ───────────────────────────────────────

    #[test]
    fn authorize_and_grant_approval_persists_and_emits() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(authorize_and_grant_approval(env, &owner, token_id, &operator).is_ok());
            assert_eq!(token_approval::get_approval(env, token_id), Some(operator));
        });
    }

    #[test]
    fn authorize_and_revoke_approval_clears_and_emits() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(authorize_and_grant_approval(env, &owner, token_id, &operator).is_ok());
            let revoked = authorize_and_revoke_approval(env, &owner, token_id).unwrap();
            assert_eq!(revoked, operator);
            assert_eq!(token_approval::get_approval(env, token_id), None);
        });
    }

    #[test]
    fn authorize_and_revoke_approval_rejects_unauthorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let stranger = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            token_approval::save_approval(env, token_id, &operator);

            assert_eq!(
                authorize_and_revoke_approval(env, &stranger, token_id),
                Err(Error::Unauthorized)
            );
            // Approval is still intact
            assert_eq!(token_approval::get_approval(env, token_id), Some(operator));
        });
    }
}
