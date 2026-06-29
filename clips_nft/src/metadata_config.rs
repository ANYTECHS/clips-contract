//! Maximum metadata size configuration.
//!
//! Stores the maximum allowed size of metadata (in bytes) accepted during NFT minting.
//!
//! # Storage
//! Key: `DataKey::MaxMetadataSize` (instance storage)
//!
//! # Limits
//! Must be a positive value (greater than 0).

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error};

/// Default maximum metadata size: 100 KB = 102400 bytes.
pub const DEFAULT_MAX_METADATA_SIZE: u32 = 102400;

/// Store the maximum metadata size in bytes.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if `size == 0`.
pub fn set_max_metadata_size(env: &Env, size: u32) -> Result<(), Error> {
    if size == 0 {
        return Err(Error::InvalidConfig);
    }
    env.storage().instance().set(&DataKey::MaxMetadataSize, &size);
    Ok(())
}

/// Return the maximum metadata size in bytes.
///
/// Falls back to [`DEFAULT_MAX_METADATA_SIZE`] (102400) if never explicitly set.
pub fn get_max_metadata_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MaxMetadataSize)
        .unwrap_or(DEFAULT_MAX_METADATA_SIZE)
}

/// Validate that a metadata URI does not exceed the maximum allowed size.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] if the metadata size exceeds the limit.
pub fn validate_metadata_size(env: &Env, metadata_uri: &String) -> Result<(), Error> {
    let max_size = get_max_metadata_size(env);
    if metadata_uri.len() > max_size {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn get_default_max_metadata_size() {
        let env = Env::default();
        assert_eq!(get_max_metadata_size(&env), DEFAULT_MAX_METADATA_SIZE);
    }

    #[test]
    fn set_and_get_max_metadata_size() {
        let env = Env::default();
        let new_size = 204800;

        set_max_metadata_size(&env, new_size).unwrap();
        assert_eq!(get_max_metadata_size(&env), new_size);
    }

    #[test]
    fn set_zero_size_returns_error() {
        let env = Env::default();
        assert_eq!(set_max_metadata_size(&env, 0), Err(Error::InvalidConfig));
    }

    #[test]
    fn validate_metadata_size_under_limit() {
        let env = Env::default();
        let metadata = String::from_str(&env, "small metadata");
        assert!(validate_metadata_size(&env, &metadata).is_ok());
    }

    #[test]
    fn validate_metadata_size_over_limit() {
        let env = Env::default();
        set_max_metadata_size(&env, 10).unwrap();
        // Create a string longer than 10 chars
        let metadata = String::from_str(&env, "this is way too long");
        assert_eq!(validate_metadata_size(&env, &metadata), Err(Error::InvalidConfig));
    }
}
