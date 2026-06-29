//! Config storage struct.
//!
//! Stores global contract configuration: platform fee, max royalty,
//! paused state, and version.
//!
//! # Storage
//! Key: `DataKey::Config` (instance storage)

use soroban_sdk::{contracttype, Env};

use crate::types::{DataKey, Error};

/// Global contract configuration.
#[contracttype]
#[derive(Clone)]
pub struct Config {
    /// Platform fee in basis points (0–1 000).
    pub platform_fee_bps: u32,
    /// Maximum royalty in basis points (0–10 000).
    pub max_royalty_bps: u32,
    /// When `true`, state-changing operations are blocked.
    pub paused: bool,
    /// Contract version (monotonically increasing).
    pub version: u32,
}

/// Persist the [`Config`] to instance storage.
pub fn save_config(env: &Env, config: &Config) {
    env.storage().instance().set(&DataKey::Config, config);
}

/// Return the stored [`Config`].
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no config has been stored.
pub fn get_config(env: &Env) -> Result<Config, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(Error::NotInitialized)
}
