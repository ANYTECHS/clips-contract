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
