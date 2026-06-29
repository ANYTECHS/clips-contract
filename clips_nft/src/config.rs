//! Global contract configuration.
//!
//! [`Config`] consolidates all top-level settings into a single storable
//! struct so callers can read or update the contract state in one round-trip.

use soroban_sdk::{contracttype, Address, Env, String};

use crate::types::{DataKey, Error};

/// Contract version constant — bump on breaking interface changes.
pub const CONTRACT_VERSION: u32 = 1;

/// Default and limit constants for batch/collection size.
pub const MAX_BATCH_MINT_SIZE: u32 = 50;
pub const MAX_COLLECTION_SIZE: u32 = 10_000;

/// Reusable struct that holds every global contract setting.
///
/// Stored under [`DataKey::Config`] in instance storage.
#[contracttype]
#[derive(Clone)]
pub struct Config {
    /// Contract owner / administrator address.
    pub owner: Address,
    /// Semantic version number (monotonically increasing integer).
    pub version: u32,
    /// Platform fee in basis points (0–1 000, i.e. 0 %–10 %).
    pub platform_fee_bps: u32,
    /// Default royalty in basis points applied to newly minted NFTs (0–10 000).
    pub default_royalty_bps: u32,
    /// When `true`, mint and transfer operations are blocked.
    pub paused: bool,
    /// Maximum number of NFTs mintable in a single batch call (1–100).
    pub max_batch_mint_size: u32,
    /// Maximum total NFTs in a collection (1–100 000).
    pub max_collection_size: u32,
}

/// Event emitted whenever a config value changes.
#[contracttype]
#[derive(Clone)]
pub struct ConfigUpdateEvent {
    pub key: String,
    pub old_value: u32,
    pub new_value: u32,
    pub updater: Address,
}

/// Persist a [`Config`] snapshot to instance storage, emitting events for changed fields.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when fee or royalty limits are exceeded,
/// or [`Error::InvalidConfig`] when batch/collection size limits are violated.
pub fn set_config(env: &Env, config: Config, updater: Address) -> Result<(), Error> {
    if config.platform_fee_bps > crate::platform_fee::MAX_PLATFORM_FEE_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if config.default_royalty_bps > crate::default_royalty::MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if config.max_batch_mint_size < 1 || config.max_batch_mint_size > 100 {
        return Err(Error::InvalidConfig);
    }
    if config.max_collection_size < 1 || config.max_collection_size > 100_000 {
        return Err(Error::InvalidConfig);
    }

    let old = get_config(env);

    // Emit events for changed u32 fields.
    if let Some(ref old) = old {
        emit_if_changed(env, &updater, "platform_fee_bps", old.platform_fee_bps, config.platform_fee_bps);
        emit_if_changed(env, &updater, "default_royalty_bps", old.default_royalty_bps, config.default_royalty_bps);
        emit_if_changed(env, &updater, "max_batch_mint_size", old.max_batch_mint_size, config.max_batch_mint_size);
        emit_if_changed(env, &updater, "max_collection_size", old.max_collection_size, config.max_collection_size);
    }

    env.storage().instance().set(&DataKey::Config, &config);
    Ok(())
}

fn emit_if_changed(env: &Env, updater: &Address, key: &str, old_value: u32, new_value: u32) {
    if old_value != new_value {
        env.events().publish(
            ("config_update",),
            ConfigUpdateEvent {
                key: String::from_str(env, key),
                old_value,
                new_value,
                updater: updater.clone(),
            },
        );
    }
}

/// Return the stored [`Config`], or `None` if the contract is not yet initialized.
pub fn get_config(env: &Env) -> Option<Config> {
    env.storage().instance().get(&DataKey::Config)
}

/// Reusable service for reading, validating, updating config and emitting events.
pub struct ConfigService;

impl ConfigService {
    pub fn read_config(env: &Env) -> Option<Config> {
        get_config(env)
    }

    pub fn validate_update(config: &Config) -> Result<(), Error> {
        if config.platform_fee_bps > crate::platform_fee::MAX_PLATFORM_FEE_BPS {
            return Err(Error::InvalidBasisPoints);
        }
        if config.default_royalty_bps > crate::default_royalty::MAX_ROYALTY_BPS {
            return Err(Error::InvalidBasisPoints);
        }
        if config.max_batch_mint_size < 1 || config.max_batch_mint_size > 100 {
            return Err(Error::InvalidConfig);
        }
        if config.max_collection_size < 1 || config.max_collection_size > 100_000 {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }

    pub fn update_config(env: &Env, config: Config, updater: Address) -> Result<(), Error> {
        Self::validate_update(&config)?;
        set_config(env, config, updater)
    }
}
