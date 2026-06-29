//! Default royalty configuration.
//!
//! Stores a contract-wide default royalty percentage (in basis points) that is
//! applied to newly minted NFTs when no per-token royalty is explicitly
//! provided.
//!
//! # Storage
//! Key: `DataKey::DefaultRoyaltyBps` (instance storage)
//!
//! # Limits
//! `0` – `10_000` bps inclusive (0 % – 100 %).  
//! Typical values are in the range 100–1 000 bps (1 %–10 %).

use soroban_sdk::Env;

use crate::types::{DataKey, Error};

pub use crate::storage_constants::{DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS};

/// Persist the default royalty in basis points.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when `bps > MAX_ROYALTY_BPS`.
pub fn set_default_royalty_bps(env: &Env, bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    env.storage().instance().set(&DataKey::DefaultRoyaltyBps, &bps);
    Ok(())
}

/// Return the stored default royalty in basis points.
///
/// Falls back to [`DEFAULT_ROYALTY_BPS`] (500) if never explicitly set.
pub fn get_default_royalty_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::DefaultRoyaltyBps)
        .unwrap_or(DEFAULT_ROYALTY_BPS)
}
