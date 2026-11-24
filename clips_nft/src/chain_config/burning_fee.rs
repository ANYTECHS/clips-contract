//! Burning Fee Configuration — resolves issue #1133.
//!
//! Stores and validates configurable fees charged whenever an NFT is burned.

use soroban_sdk::Env;

use crate::types::Error;
use super::keys::ConfigKey;

/// Maximum allowed burning fee.
pub const MAX_BURNING_FEE: i128 = 10_000_000_000;

/// Return the configured burning fee, defaulting to 0.
pub fn get_burning_fee(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ConfigKey::BurningFee)
        .unwrap_or(0)
}

/// Set the burning fee after validating that it is non-negative and within limits.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if fee is negative or exceeds [`MAX_BURNING_FEE`].
pub fn set_burning_fee(env: &Env, fee: i128) -> Result<(), Error> {
    if fee < 0 || fee > MAX_BURNING_FEE {
        return Err(Error::InvalidConfig);
    }
    env.storage().instance().set(&ConfigKey::BurningFee, &fee);
    Ok(())
}
