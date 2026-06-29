//! Storage access guard (Task 3).
//!
//! Reusable authorization checks that must pass before any storage
//! mutation is allowed. Each function returns a typed [`Error`] so
//! callers can propagate failures cleanly.

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenData, TokenId};

/// Verify the caller is the stored admin and call `require_auth`.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract not yet initialised.
/// - [`Error::Unauthorized`] — caller is not the admin.
pub fn guard_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    if *caller != admin {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

/// Verify the contract is not paused.
///
/// # Errors
/// - [`Error::ContractPaused`] — contract is currently paused.
pub fn guard_not_paused(env: &Env) -> Result<(), Error> {
    if env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
    {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

/// Verify the caller is the owner of `token_id` and call `require_auth`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — caller is not the token owner.
pub fn guard_token_owner(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::TokenNotFound)?;
    if data.owner != *caller {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};
    use crate::types::{DataKey, TokenData};

    #[test]
    fn test_guard_admin_not_initialized() {
        let env = Env::default();
        let addr = Address::generate(&env);
        assert_eq!(guard_admin(&env, &addr), Err(Error::NotInitialized));
    }

    #[test]
    fn test_guard_admin_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        assert_eq!(guard_admin(&env, &other), Err(Error::Unauthorized));
    }

    #[test]
    fn test_guard_admin_success() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        assert!(guard_admin(&env, &admin).is_ok());
    }

    #[test]
    fn test_guard_not_paused_when_paused() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::Paused, &true);
        assert_eq!(guard_not_paused(&env), Err(Error::ContractPaused));
    }

    #[test]
    fn test_guard_not_paused_when_unpaused() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::Paused, &false);
        assert!(guard_not_paused(&env).is_ok());
    }

    #[test]
    fn test_guard_token_owner_not_owner() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Token(0), &TokenData { owner: owner.clone(), clip_id: 1 });
        assert_eq!(guard_token_owner(&env, &other, 0), Err(Error::Unauthorized));
    }

    #[test]
    fn test_guard_token_owner_success() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Token(0), &TokenData { owner: owner.clone(), clip_id: 1 });
        assert!(guard_token_owner(&env, &owner, 0).is_ok());
    }
}
