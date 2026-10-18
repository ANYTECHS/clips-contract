//! Treasury wallet storage.
//!
//! Stores the treasury wallet responsible for receiving platform fees.
//!
//! # Storage
//! Key: `DataKey::Treasury` (instance storage)

use soroban_sdk::{Address, Env};

use crate::types::DataKey;

/// Save the treasury wallet. Overwrites any existing value (also used for updates).
pub fn set_treasury_wallet(env: &Env, wallet: &Address) {
    env.storage().instance().set(&DataKey::Treasury, wallet);
}

/// Return the treasury wallet, or `None` if not set.
pub fn get_treasury_wallet(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Treasury)
}
