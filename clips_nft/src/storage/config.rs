//! Config storage helpers — resolves issue #492.

use soroban_sdk::{panic_with_error, Env};

use crate::{
    storage::keys::StorageKey,
    types::{Config, Error},
};

const MAX_BPS: u32 = 10_000;

/// Read the stored [`Config`]. Panics with [`Error::NotInitialized`] if absent.
pub fn get_config(env: &Env) -> Config {
    if let Some(cfg) = env.storage().instance().get(&StorageKey::Config) {
        cfg
    } else {
        panic_with_error!(env, Error::NotInitialized)
    }
}

/// Persist a [`Config`] to instance storage.
pub fn set_config(env: &Env, config: &Config) {
    env.storage().instance().set(&StorageKey::Config, config);
}

/// Validate config values. Returns `Err(InvalidBasisPoints)` on bad inputs.
pub fn validate_config(config: &Config) -> Result<(), Error> {
    if config.max_royalty_bps > MAX_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if config.platform_fee_bps > MAX_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    Ok(())
}
