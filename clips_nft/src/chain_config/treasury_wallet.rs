//! Treasury Wallet Configuration — resolves issue #1134.
//!
//! Stores and validates the wallet address that receives platform fees.

use soroban_sdk::{Address, Env};

use crate::types::Error;
use super::keys::ConfigKey;

/// Retrieve the treasury wallet address from instance storage.
pub fn get_treasury_wallet(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ConfigKey::TreasuryWallet)
}

/// Save the treasury wallet address to instance storage after validation.
pub fn set_treasury_wallet(env: &Env, wallet: &Address) -> Result<(), Error> {
    env.storage()
        .instance()
        .set(&ConfigKey::TreasuryWallet, wallet);
    Ok(())
}
