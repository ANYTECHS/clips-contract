//! Global configuration storage module — resolves issue #1124.
//!
//! Serves as the central source of truth for configurable contract settings.
//! Manages persistent/instance storage for global contract configuration.

use soroban_sdk::{contracttype, Address, Env};

use crate::types::Error;
use super::keys::ConfigKey;

/// Global configuration settings layout.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalConfiguration {
    /// Contract owner / administrator address.
    pub admin: Address,
    /// Platform fee in basis points (0–1_000).
    pub platform_fee_bps: u32,
    /// Marketplace fee in basis points (0–1_000).
    pub marketplace_fee_bps: u32,
    /// Maximum NFTs mintable per batch call (1–100).
    pub max_batch_mint_size: u32,
    /// Maximum total size of the collection (1–100_000).
    pub max_collection_size: u32,
    /// Contract pause status.
    pub paused: bool,
}

/// Retrieve the global configuration from instance storage.
pub fn get_global_configuration(env: &Env) -> Option<GlobalConfiguration> {
    env.storage().instance().get(&ConfigKey::GlobalConfig)
}

/// Persist the global configuration in instance storage.
pub fn set_global_configuration(env: &Env, config: &GlobalConfiguration) -> Result<(), Error> {
    if config.max_batch_mint_size < 1 || config.max_batch_mint_size > 100 {
        return Err(Error::InvalidConfig);
    }
    if config.max_collection_size < 1 || config.max_collection_size > 100_000 {
        return Err(Error::InvalidConfig);
    }
    env.storage().instance().set(&ConfigKey::GlobalConfig, config);
    Ok(())
}
