//! Platform fee configuration.
//!
//! Stores the platform fee in basis points (bps) that applies to all
//! marketplace transactions. Maximum allowed fee is 1 000 bps (10 %).
//!
//! # Storage
//! Key: `DataKey::PlatformFee` (instance storage)
//!
//! # Limits
//! `0` – `1_000` bps inclusive (0 % – 10 %)

use soroban_sdk::Env;

use crate::types::{DataKey, Error};

/// Maximum platform fee: 10 % = 1 000 basis points.
pub const MAX_PLATFORM_FEE_BPS: u32 = 1_000;

/// Store the platform fee in basis points.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] if `fee_bps > MAX_PLATFORM_FEE_BPS`.
pub fn set_platform_fee(env: &Env, fee_bps: u32) -> Result<(), Error> {
    if fee_bps > MAX_PLATFORM_FEE_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    env.storage().instance().set(&DataKey::PlatformFee, &fee_bps);
    Ok(())
}

/// Return the current platform fee in basis points (defaults to `0`).
pub fn get_platform_fee(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::PlatformFee)
        .unwrap_or(0)
}
