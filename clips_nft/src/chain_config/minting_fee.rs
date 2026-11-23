//! Minting Fee Configuration — resolves issue #1132.
//!
//! Stores and validates configurable fees charged whenever an NFT is minted.

use soroban_sdk::Env;

use crate::types::Error;
use super::keys::ConfigKey;

/// Maximum allowed minting fee (10,000,000 stroops or units).
pub const MAX_MINTING_FEE: i128 = 10_000_000_000;

/// Return the configured minting fee, defaulting to 0.
pub fn get_minting_fee(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ConfigKey::MintingFee)
        .unwrap_or(0)
}

/// Set the minting fee after validating that it is non-negative and within limits.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if fee is negative or exceeds [`MAX_MINTING_FEE`].
pub fn set_minting_fee(env: &Env, fee: i128) -> Result<(), Error> {
    if fee < 0 || fee > MAX_MINTING_FEE {
        return Err(Error::InvalidConfig);
    }
    env.storage().instance().set(&ConfigKey::MintingFee, &fee);
    Ok(())
}
