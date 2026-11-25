//! Authorization helper utilities (issue #1246).
//!
//! Reusable caller/resource checks without duplicated authorization logic.
//! Every helper delegates to the existing authoritative guards and returns
//! [`AuthorizationResult`](super::result::AuthorizationResult), so failures
//! always carry the same contract [`Error`](crate::types::Error)s
//! (`Unauthorized`, `TokenNotFound`) no matter which helper is used.

use soroban_sdk::{Address, Env};

use super::result::AuthorizationResult;
use crate::{
    admin_access_control_guard, blacklist, operator_approval_guard, ownership_guard,
    token_owner_storage,
    types::{Error, TokenId},
};

/// Check that `caller` may act at all: demands an auth envelope and rejects
/// blacklisted callers with `Error::Unauthorized`.
pub fn check_caller(env: &Env, caller: &Address) -> AuthorizationResult {
    caller.require_auth();
    if blacklist::is_blacklisted(env, caller) {
        return AuthorizationResult::Denied(Error::Unauthorized);
    }
    AuthorizationResult::Allowed
}

/// Check that `caller` owns `token_id`. Delegates to
/// `ownership_guard::require_owner`: missing token → `TokenNotFound`,
/// non-owner → `Unauthorized`.
pub fn check_resource_owner(env: &Env, caller: &Address, token_id: TokenId) -> AuthorizationResult {
    ownership_guard::require_owner(env, caller, token_id).into()
}

/// Check that `caller` may operate on `token_id` as owner, approved
/// operator, or admin — in that order. Missing token → `TokenNotFound`;
/// otherwise a non-privileged caller → `Unauthorized`.
///
/// Uses the read-only probes (no duplicated logic); demands one auth
/// envelope up front.
pub fn check_owner_operator_or_admin(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> AuthorizationResult {
    caller.require_auth();
    if !token_owner_storage::has_owner(env, token_id) {
        return AuthorizationResult::Denied(Error::TokenNotFound);
    }
    if ownership_guard::check_caller_is_owner(env, caller, token_id) {
        return AuthorizationResult::Allowed;
    }
    let owner = match token_owner_storage::get_owner(env, token_id) {
        Ok(owner) => owner,
        Err(_) => return AuthorizationResult::Denied(Error::TokenNotFound),
    };
    if operator_approval_guard::check_operator_approved(env, caller, &owner, token_id) {
        return AuthorizationResult::Allowed;
    }
    if admin_access_control_guard::check_caller_is_admin(env, caller) {
        return AuthorizationResult::Allowed;
    }
    AuthorizationResult::Denied(Error::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

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

    #[test]
    fn caller_check_allows_non_blacklisted_caller() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(check_caller(env, &caller), AuthorizationResult::Allowed);
        });
    }

    #[test]
    fn caller_check_denies_blacklisted_caller() {
        with_contract(|env| {
            let caller = Address::generate(env);
            crate::blacklist::add_wallet(env, &caller);
            assert_eq!(
                check_caller(env, &caller),
                AuthorizationResult::Denied(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn resource_owner_check_allows_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                check_resource_owner(env, &owner, 1),
                AuthorizationResult::Allowed
            );
        });
    }

    #[test]
    fn resource_owner_check_denies_stranger_with_unauthorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                check_resource_owner(env, &stranger, 1),
                AuthorizationResult::Denied(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn resource_owner_check_denies_missing_token_with_not_found() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                check_resource_owner(env, &caller, 999),
                AuthorizationResult::Denied(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn owner_operator_admin_allows_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                check_owner_operator_or_admin(env, &owner, 1),
                AuthorizationResult::Allowed
            );
        });
    }

    #[test]
    fn owner_operator_admin_denies_stranger_consistently() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            let result = check_owner_operator_or_admin(env, &stranger, 1);
            assert_eq!(result, AuthorizationResult::Denied(Error::Unauthorized));
            assert_eq!(result.into_contract_result(), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn owner_operator_admin_denies_missing_token_with_not_found() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                check_owner_operator_or_admin(env, &caller, 999),
                AuthorizationResult::Denied(Error::TokenNotFound)
            );
        });
    }
}
