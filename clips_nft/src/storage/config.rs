//! Contract configuration persistence.
//!
//! The [`Config`](crate::types::Config) record lives in instance storage
//! under `DataKey::Config`. Writes go through `config_guard` at the entry
//! point; this module owns only the bytes.

use soroban_sdk::Env;

use crate::types::{Config, DataKey, Error};

/// Persist the configuration record.
pub fn set_config(env: &Env, config: &Config) {
    env.storage().instance().set(&DataKey::Config, config);
}

/// Load the configuration record. Panics when the contract was never
/// initialized (`init` always writes it).
pub fn get_config(env: &Env) -> Config {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .expect("config not initialized")
}

/// Validate configuration values without writing storage.
pub fn validate_config(config: &Config) -> Result<(), Error> {
    if config.max_royalty_bps > 10_000 || config.platform_fee_bps > 10_000 {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}
