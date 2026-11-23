//! Maximum Batch Mint Size Configuration — resolves issue #1142.
//!
//! Stores and validates the maximum number of NFTs mintable in a single transaction.

use soroban_sdk::Env;

use crate::types::Error;
use super::keys::ConfigKey;

/// Default maximum batch mint size.
pub const DEFAULT_MAX_BATCH_MINT_SIZE: u32 = 50;
/// Minimum allowed batch mint size.
pub const MIN_BATCH_MINT_SIZE_LIMIT: u32 = 1;
/// Maximum allowed batch mint size.
pub const MAX_BATCH_MINT_SIZE_LIMIT: u32 = 100;

/// Return the configured maximum batch mint size, defaulting to [`DEFAULT_MAX_BATCH_MINT_SIZE`].
pub fn get_max_batch_mint_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&ConfigKey::MaxBatchMintSize)
        .unwrap_or(DEFAULT_MAX_BATCH_MINT_SIZE)
}

/// Set the maximum batch mint size after validating bounds.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if `size` is outside 1..=100.
pub fn set_max_batch_mint_size(env: &Env, size: u32) -> Result<(), Error> {
    if size < MIN_BATCH_MINT_SIZE_LIMIT || size > MAX_BATCH_MINT_SIZE_LIMIT {
        return Err(Error::InvalidConfig);
    }
    env.storage()
        .instance()
        .set(&ConfigKey::MaxBatchMintSize, &size);
    Ok(())
}
