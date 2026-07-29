//! Contract owner storage.
//!
//! Stores and retrieves the contract owner address.
//!
//! # Storage
//! Key: `DataKey::Admin` (instance storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error};

/// Persist the contract owner address.
pub fn save_owner(env: &Env, owner: &Address) {
    env.storage().instance().set(&DataKey::Admin, owner);
}

/// Return the contract owner address.
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no owner has been stored.
pub fn get_owner(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

/// Update the contract owner address.
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no owner has been stored.
pub fn update_owner(env: &Env, new_owner: &Address) -> Result<(), Error> {
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }
    env.storage().instance().set(&DataKey::Admin, new_owner);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn test_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_save_and_get_owner() {
        let env = test_env();
        let owner = Address::generate(&env);

        save_owner(&env, &owner);
        assert_eq!(get_owner(&env), Ok(owner));
    }

    #[test]
    fn test_get_owner_not_initialized() {
        let env = test_env();
        assert_eq!(get_owner(&env), Err(Error::NotInitialized));
    }

    #[test]
    fn test_update_owner_success() {
        let env = test_env();
        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);

        save_owner(&env, &owner_a);
        assert_eq!(update_owner(&env, &owner_b), Ok(()));
        assert_eq!(get_owner(&env), Ok(owner_b));
    }

    #[test]
    fn test_update_owner_not_initialized() {
        let env = test_env();
        let owner = Address::generate(&env);
        assert_eq!(update_owner(&env, &owner), Err(Error::NotInitialized));
    }
}
