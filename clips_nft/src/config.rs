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
    crate::config_guard::require_config_admin(env, &updater)?;

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

    env.storage().instance().set(&DataKey::Config, &config);

    // Publish only after persistence, so observers can rely on storage matching
    // the values reported by the event.
    if let Some(ref old) = old {
        let timestamp = env.ledger().timestamp();
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
            ConfigField::DefaultRoyalty,
            old.default_royalty_bps as u64,
            config.default_royalty_bps as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::BatchMintSize,
            old.max_batch_mint_size as u64,
            config.max_batch_mint_size as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::BatchTransferSize,
            old.max_batch_transfer_size as u64,
            config.max_batch_transfer_size as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::MaxCollectionSize,
            old.max_collection_size as u64,
            config.max_collection_size as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_numeric_change(
            env,
            ConfigField::ContractVersion,
            old.version as u64,
            config.version as u64,
            &updater,
            timestamp,
        );
        config_updated_event::emit_boolean_change(
            env,
            ConfigField::Paused,
            old.paused,
            config.paused,
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
        set_config(env, config, updater)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event_topics::TOPIC_CONFIG_UPDATED,
        types::{ConfigUpdatedEvent, ConfigValue},
        AtomicMintContract, ClipCashNFT, ClipCashNFTClient, ClipsNftContract,
        ClipsNftContractClient,
    };
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger},
        xdr::ContractEventBody,
        Address, Env, Symbol, TryFromVal,
    };

    fn config_updates(env: &Env) -> std::vec::Vec<ConfigUpdatedEvent> {
        env.events()
            .all()
            .events()
            .iter()
            .filter_map(|event| {
                if let ContractEventBody::V0(v0) = &event.body {
                    if v0.topics.len() == 1
                        && Symbol::try_from_val(env, &v0.topics[0])
                            == Ok(TOPIC_CONFIG_UPDATED)
                    {
                        return ConfigUpdatedEvent::try_from_val(env, &v0.data).ok();
                    }
                }
                None
            })
            .collect()
    }

    #[test]
    fn config_snapshot_emits_one_event_per_changed_setting() {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        let admin = Address::generate(&env);
        env.mock_all_auths();

        env.as_contract(&contract_id, || {
            env.storage()
                .instance()
                .set(&DataKey::Admin, &admin);

            let initial = Config {
                owner: admin.clone(),
                version: 1,
                platform_fee_bps: 100,
                default_royalty_bps: 500,
                paused: false,
                max_batch_mint_size: 10,
                max_batch_transfer_size: 20,
                max_collection_size: 100,
            };
            set_config(&env, initial, admin.clone()).unwrap();

            env.ledger().set_timestamp(1_700_000_000);
            let updated = Config {
                owner: admin.clone(),
                version: 2,
                platform_fee_bps: 200,
                default_royalty_bps: 750,
                paused: true,
                max_batch_mint_size: 15,
                max_batch_transfer_size: 25,
                max_collection_size: 200,
            };
            set_config(&env, updated.clone(), admin.clone()).unwrap();

            let updates = config_updates(&env);
            assert_eq!(updates.len(), 7);
            assert!(updates.iter().all(|event| {
                event.admin == admin && event.timestamp == 1_700_000_000
            }));
            assert!(updates.contains(&ConfigUpdatedEvent {
                field: ConfigField::PlatformFee,
                previous_value: ConfigValue::Number(100),
                new_value: ConfigValue::Number(200),
                admin: admin.clone(),
                timestamp: 1_700_000_000,
            }));
            assert!(updates.contains(&ConfigUpdatedEvent {
                field: ConfigField::DefaultRoyalty,
                previous_value: ConfigValue::Number(500),
                new_value: ConfigValue::Number(750),
                admin: admin.clone(),
                timestamp: 1_700_000_000,
            }));
            assert!(updates.iter().any(|event| {
                event.field == ConfigField::BatchMintSize
                    && event.previous_value == ConfigValue::Number(10)
                    && event.new_value == ConfigValue::Number(15)
            }));
            assert!(updates.iter().any(|event| {
                event.field == ConfigField::BatchTransferSize
                    && event.previous_value == ConfigValue::Number(20)
                    && event.new_value == ConfigValue::Number(25)
            }));
            assert!(updates.iter().any(|event| {
                event.field == ConfigField::MaxCollectionSize
                    && event.previous_value == ConfigValue::Number(100)
                    && event.new_value == ConfigValue::Number(200)
            }));
            assert!(updates.iter().any(|event| {
                event.field == ConfigField::ContractVersion
                    && event.previous_value == ConfigValue::Number(1)
                    && event.new_value == ConfigValue::Number(2)
            }));
            assert!(updates.contains(&ConfigUpdatedEvent {
                field: ConfigField::Paused,
                previous_value: ConfigValue::Boolean(false),
                new_value: ConfigValue::Boolean(true),
                admin: admin.clone(),
                timestamp: 1_700_000_000,
            }));

            set_config(&env, updated, admin).unwrap();
            assert_eq!(config_updates(&env).len(), 7);
        });
    }

    #[test]
    fn legacy_config_setter_emits_changed_fee_royalty_and_cooldown() {
        let env = Env::default();
        let contract_id = env.register(ClipCashNFT, ());
        let client = ClipCashNFTClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.mock_all_auths();

        client.init(&admin);
        let initial = crate::types::Config {
            admin: admin.clone(),
            max_royalty_bps: 500,
            mint_cooldown_secs: 0,
            platform_fee_bps: 0,
        };
        client.set_config(&admin, &initial).unwrap();

        env.ledger().set_timestamp(1_700_000_002);
        let updated = crate::types::Config {
            admin: admin.clone(),
            max_royalty_bps: 750,
            mint_cooldown_secs: 30,
            platform_fee_bps: 200,
        };
        client.set_config(&admin, &updated).unwrap();

        let updates = config_updates(&env);
        assert_eq!(updates.len(), 3);
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::PlatformFee,
            previous_value: ConfigValue::Number(0),
            new_value: ConfigValue::Number(200),
            admin: admin.clone(),
            timestamp: 1_700_000_002,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::MaxRoyalty,
            previous_value: ConfigValue::Number(500),
            new_value: ConfigValue::Number(750),
            admin: admin.clone(),
            timestamp: 1_700_000_002,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::MintCooldown,
            previous_value: ConfigValue::Number(0),
            new_value: ConfigValue::Number(30),
            admin,
            timestamp: 1_700_000_002,
        }));
    }

    #[test]
    fn standalone_config_changes_emit_previous_and_new_values() {
        let env = Env::default();
        let contract_id = env.register(ClipsNftContract, ());
        let client = ClipsNftContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.mock_all_auths();

        client.init(&admin).unwrap();
        env.ledger().set_timestamp(1_700_000_001);
        client.set_platform_fee(&admin, &200).unwrap();
        client.set_default_royalty_bps(&admin, &750).unwrap();
        client.set_max_royalty_bps(&admin, &9_000).unwrap();
        client.set_max_metadata_size(&admin, &204_800).unwrap();
        let asset = Address::generate(&env);
        client.add_currency(&admin, &asset).unwrap();
        client.remove_currency(&admin, &asset).unwrap();

        let updates = config_updates(&env);
        assert_eq!(updates.len(), 6);
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::PlatformFee,
            previous_value: ConfigValue::Number(0),
            new_value: ConfigValue::Number(200),
            admin: admin.clone(),
            timestamp: 1_700_000_001,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::DefaultRoyalty,
            previous_value: ConfigValue::Number(
                crate::default_royalty::DEFAULT_ROYALTY_BPS as u64
            ),
            new_value: ConfigValue::Number(750),
            admin: admin.clone(),
            timestamp: 1_700_000_001,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::MaxMetadataSize,
            previous_value: ConfigValue::Number(
                crate::metadata_config::DEFAULT_MAX_METADATA_SIZE as u64
            ),
            new_value: ConfigValue::Number(204_800),
            admin: admin.clone(),
            timestamp: 1_700_000_001,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::MaxRoyalty,
            previous_value: ConfigValue::Number(
                crate::storage_constants::MAX_ROYALTY_BPS as u64
            ),
            new_value: ConfigValue::Number(9_000),
            admin: admin.clone(),
            timestamp: 1_700_000_001,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::SupportedAsset,
            previous_value: ConfigValue::Unset,
            new_value: ConfigValue::Address(asset.clone()),
            admin: admin.clone(),
            timestamp: 1_700_000_001,
        }));
        assert!(updates.contains(&ConfigUpdatedEvent {
            field: ConfigField::SupportedAsset,
            previous_value: ConfigValue::Address(asset),
            new_value: ConfigValue::Unset,
            admin,
            timestamp: 1_700_000_001,
        }));
    }
}
