//! Maximum Collection Size Configuration — resolves issue #1143.
//!
//! Stores and validates the maximum number of NFTs that can exist within the collection.

use soroban_sdk::Env;

use crate::types::Error;
use super::keys::ConfigKey;

/// Default maximum collection size.
pub const DEFAULT_MAX_COLLECTION_SIZE: u32 = 10_000;
/// Minimum allowed collection size.
pub const MIN_COLLECTION_SIZE_LIMIT: u32 = 1;
/// Maximum allowed collection size.
pub const MAX_COLLECTION_SIZE_LIMIT: u32 = 100_000;

/// Return the configured maximum collection size, defaulting to [`DEFAULT_MAX_COLLECTION_SIZE`].
pub fn get_max_collection_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&ConfigKey::MaxCollectionSize)
        .unwrap_or(DEFAULT_MAX_COLLECTION_SIZE)
}

/// Set the maximum collection size after validating bounds.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if `size` is outside 1..=100_000.
pub fn set_max_collection_size(env: &Env, size: u32) -> Result<(), Error> {
    if size < MIN_COLLECTION_SIZE_LIMIT || size > MAX_COLLECTION_SIZE_LIMIT {
        return Err(Error::InvalidConfig);
    }
    env.storage()
        .instance()
        .set(&ConfigKey::MaxCollectionSize, &size);
    Ok(())
}
