//! Owner verification guard — restrict operations to token owners (issue #1091).
//!
//! This guard module re-exports the existing ownership functionality from
//! [`crate::ownership_guard`] as part of the centralized guards framework.
//!
//! For detailed documentation, see [`crate::ownership_guard`].
//!
//! In addition to the free functions, [`OwnerGuard`] implements the common
//! [`Guard`] trait so ownership checks can be composed with other guards.

pub use crate::ownership_guard::{
    check_caller_is_owner, get_owner_for_token, require_owner,
};

use crate::guards::{Guard, GuardContext, GuardResult};

/// Guard that restricts an operation to the owner of the token in the context.
///
/// The guard reads the caller and token id from the [`GuardContext`] and
/// verifies that the caller is the stored owner of that token.
#[derive(Clone, Copy, Debug, Default)]
pub struct OwnerGuard;

impl OwnerGuard {
    /// Create a new owner guard.
    pub const fn new() -> Self {
        Self
    }
}

impl Guard for OwnerGuard {
    fn check(&self, ctx: &GuardContext) -> GuardResult {
        if check_caller_is_owner(ctx.env, ctx.token_id, &ctx.caller) {
            GuardResult::Allow
        } else {
            GuardResult::Deny(crate::types::Error::Unauthorized)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataKey, Error, TokenId};
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

    #[test]
    fn check_caller_is_owner_returns_true_for_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            assert!(check_caller_is_owner(env, token_id, &owner));
        });
    }

    #[test]
    fn check_caller_is_owner_returns_false_for_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            assert!(!check_caller_is_owner(env, token_id, &other));
        });
    }

    #[test]
    fn require_owner_accepts_token_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            assert!(require_owner(env, token_id, &owner).is_ok());
        });
    }

    #[test]
    fn require_owner_rejects_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            assert_eq!(require_owner(env, token_id, &other), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn get_owner_for_token_returns_stored_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            let retrieved = get_owner_for_token(env, token_id).unwrap();
            assert_eq!(retrieved, owner);
        });
    }

    #[test]
    fn owner_guard_allows_token_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            let ctx = GuardContext::new(env, token_id, owner.clone());
            assert_eq!(OwnerGuard::new().check(&ctx), GuardResult::Allow);
        });
    }

    #[test]
    fn owner_guard_denies_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            let token_id: TokenId = 1;
            let key = DataKey::TokenOwner(token_id);
            env.storage().persistent().set(&key, &owner);

            let ctx = GuardContext::new(env, token_id, other);
            assert_eq!(
                OwnerGuard::new().check(&ctx),
                GuardResult::Deny(Error::Unauthorized)
            );
        });
    }
}
