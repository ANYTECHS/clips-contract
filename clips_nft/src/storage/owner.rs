//! Contract owner storage.
//!
//! Stores and retrieves the contract owner address using [`StorageKey::Admin`]
//! in instance storage.  Every write goes through this module so the rest of
//! the codebase never touches raw storage keys for the owner record.
//!
//! # Storage
//! Key:    [`StorageKey::Admin`] (instance storage)
//! Value:  [`Address`]
//!
//! # Operations
//! - [`save_owner`]   — persist an owner address for the first time
//! - [`get_owner`]    — read the stored owner address
//! - [`update_owner`] — overwrite an existing owner address

use soroban_sdk::{Address, Env};

use crate::{storage::keys::StorageKey, types::Error};

/// Persist the contract owner address.
///
/// Calling this a second time silently overwrites the previous value; callers
/// that want one-time initialization should check [`get_owner`] first.
pub fn save_owner(env: &Env, owner: &Address) {
    env.storage()
        .instance()
        .set(&StorageKey::Admin, owner);
}

/// Return the contract owner address.
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no owner has been stored yet.
pub fn get_owner(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)
}

/// Overwrite the contract owner address.
///
/// Unlike [`save_owner`], this function refuses to write when the contract has
/// not yet been initialized — ensuring callers cannot silently create an owner
/// record through the update path.
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no owner has been stored yet.
pub fn update_owner(env: &Env, new_owner: &Address) -> Result<(), Error> {
    if !env.storage().instance().has(&StorageKey::Admin) {
        return Err(Error::NotInitialized);
    }
    env.storage()
        .instance()
        .set(&StorageKey::Admin, new_owner);
    Ok(())
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

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
        let id = env.register(AtomicMintContract, ());
        env.as_contract(&id, || f(&env))
    }
    #[ignore]
    #[test]
    fn save_then_get_returns_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, &owner);
            assert_eq!(get_owner(env), Ok(owner));
        });
    }
    #[ignore]
    #[test]
    fn get_before_save_returns_not_initialized() {
        with_contract(|env| {
            assert_eq!(get_owner(env), Err(Error::NotInitialized));
        });
    }
    #[ignore]
    #[test]
    fn update_owner_replaces_previous() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            save_owner(env, &a);
            assert_eq!(update_owner(env, &b), Ok(()));
            assert_eq!(get_owner(env), Ok(b));
        });
    }
    #[ignore]
    #[test]
    fn update_owner_without_prior_save_returns_not_initialized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(update_owner(env, &owner), Err(Error::NotInitialized));
        });
    }
}
