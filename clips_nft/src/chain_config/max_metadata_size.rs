//! Maximum Metadata Size Configuration (Issue #1139).
//!
//! Configures the maximum metadata size accepted during NFT minting.

use soroban_sdk::Env;

use crate::types::Error;
use super::keys::ConfigKey;

/// Default maximum metadata byte length.
pub const DEFAULT_MAX_METADATA_SIZE: u32 = 512;

/// Return the configured maximum metadata size, defaulting to [`DEFAULT_MAX_METADATA_SIZE`].
pub fn get_max_metadata_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&ConfigKey::MaxMetadataSize)
        .unwrap_or(DEFAULT_MAX_METADATA_SIZE)
}

/// Set the maximum metadata size after validating positive values.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if `size` is 0.
pub fn set_max_metadata_size(env: &Env, size: u32) -> Result<(), Error> {
    if size == 0 {
        return Err(Error::InvalidConfig);
    }
    env.storage()
        .instance()
        .set(&ConfigKey::MaxMetadataSize, &size);
    Ok(())
}
