//! Global contract configuration.
//!
//! [`Config`] consolidates all top-level settings into a single storable
//! struct so callers can read or update the contract state in one round-trip.

use soroban_sdk::{contracttype, Address, Env};

use crate::config_updated_event;
use crate::types::{ConfigField, DataKey, Error};

pub use crate::storage_constants::{
    CONTRACT_VERSION, MAX_BATCH_MINT_SIZE, MAX_BATCH_TRANSFER_SIZE, MAX_COLLECTION_SIZE,
};

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
    /// Maximum number of NFTs transferable in a single batch call (1–100).
    pub max_batch_transfer_size: u32,
    /// Maximum total NFTs in a collection (1–100 000).
    pub max_collection_size: u32,
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
    // Validate combined royalty + platform fee don't exceed 100%
    crate::transaction_deduction_validator::validate_total_deduction_bps(
        config.default_royalty_bps,
        config.platform_fee_bps,
    )?;
    if config.max_batch_mint_size < 1 || config.max_batch_mint_size > 100 {
        return Err(Error::InvalidConfig);
    }
    if config.max_batch_transfer_size < 1
        || config.max_batch_transfer_size > crate::storage_constants::MAX_BATCH_TRANSFER_SIZE_LIMIT
    {
        return Err(Error::InvalidConfig);
    }
    if config.max_collection_size < 1 || config.max_collection_size > 100_000 {
        return Err(Error::InvalidConfig);
    }

    let old = get_config(env);

    // Emit events for changed u32 fields.
    if let Some(ref old) = old {
        let timestamp = env.ledger().timestamp();

        // One event per setting that actually changed (issue #932). Values are
        // widened to u64 so numeric and address settings share a single event
        // shape; see `ConfigValue`.
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::PlatformFee,
            old.platform_fee_bps as u64,
            config.platform_fee_bps as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::MaxRoyalty,
            old.default_royalty_bps as u64,
            config.default_royalty_bps as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::BatchSize,
            old.max_batch_mint_size as u64,
            config.max_batch_mint_size as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::BatchSize,
            old.max_batch_transfer_size as u64,
            config.max_batch_transfer_size as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::ContractLimit,
            old.max_collection_size as u64,
            config.max_collection_size as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::ContractLimit,
            old.version as u64,
            config.version as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_address_change(
            env,
            ConfigField::Admin,
            &old.owner,
            &config.owner,
            &updater,
            timestamp,
        );
    }

    env.storage().instance().set(&DataKey::Config, &config);
    Ok(())
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
        // Validate combined royalty + platform fee don't exceed 100%
        crate::transaction_deduction_validator::validate_total_deduction_bps(
            config.default_royalty_bps,
            config.platform_fee_bps,
        )?;
        if config.max_batch_mint_size < 1 || config.max_batch_mint_size > 100 {
            return Err(Error::InvalidConfig);
        }
        if config.max_batch_transfer_size < 1
            || config.max_batch_transfer_size
                > crate::storage_constants::MAX_BATCH_TRANSFER_SIZE_LIMIT
        {
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
