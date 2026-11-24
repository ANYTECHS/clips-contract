//! Contract configuration-updated event (issue #932).
//!
//! Emitted whenever an administrator changes a contract-wide setting — platform
//! fee, maximum royalty, batch size, supported asset, or any other contract
//! limit. Each event carries the previous and the new value so an indexer can
//! reconstruct the full configuration history without replaying storage.
//!
//! # Event topic
//! `"cfg_updt"` — within the 9-character limit for [`soroban_sdk::symbol_short`].

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{ConfigField, ConfigUpdatedEvent, ConfigValue};

/// Emit the `"cfg_updt"` event after a configuration value has been persisted.
///
/// Call once per changed setting; a call that updates several settings at once
/// emits one event per field that actually changed, so subscribers never have
/// to diff values themselves.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `field`          — Setting that changed (key).
/// * `previous_value` — Value before the update (old value).
/// * `new_value`      — Value after the update (new value).
/// * `admin`          — Account that performed the update (updater address).
/// * `timestamp`      — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_config_updated(
    env: &Env,
    field: ConfigField,
    previous_value: ConfigValue,
    new_value: ConfigValue,
    admin: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("cfg_updt"),),
        ConfigUpdatedEvent {
            field,
            previous_value,
            new_value,
            admin: admin.clone(),
            timestamp,
        },
    );
}

/// Emit a `"cfg_updt"` event for a numeric setting, skipping unchanged values.
///
/// Returns `true` when an event was emitted.
pub fn emit_numeric_change(
    env: &Env,
    field: ConfigField,
    previous: u64,
    new: u64,
    admin: &Address,
    timestamp: u64,
) -> bool {
    if previous == new {
        return false;
    }
    emit_config_updated(
        env,
        field,
        ConfigValue::Number(previous),
        ConfigValue::Number(new),
        admin,
        timestamp,
    );
    true
}

/// Emit a `"cfg_updt"` event for an address setting, skipping unchanged values.
///
/// Returns `true` when an event was emitted.
pub fn emit_address_change(
    env: &Env,
    field: ConfigField,
    previous: &Address,
    new: &Address,
    admin: &Address,
    timestamp: u64,
) -> bool {
    if previous == new {
        return false;
    }
    emit_config_updated(
        env,
        field,
        ConfigValue::Address(previous.clone()),
        ConfigValue::Address(new.clone()),
        admin,
        timestamp,
    );
    true
}

/// Emit a `"cfg_updt"` event for a string setting (e.g. metadata base URI),
/// skipping unchanged values.
///
/// Returns `true` when an event was emitted.
pub fn emit_string_change(
    env: &Env,
    field: ConfigField,
    previous: &String,
    new: &String,
    admin: &Address,
    timestamp: u64,
) -> bool {
    if previous == new {
        return false;
    }
    emit_config_updated(
        env,
        field,
        ConfigValue::String(previous.clone()),
        ConfigValue::String(new.clone()),
        admin,
        timestamp,
    );
    true
}

/// Emit a `"cfg_updt"` event for a boolean setting (e.g. paused flag),
/// skipping unchanged values.
///
/// Returns `true` when an event was emitted.
pub fn emit_bool_change(
    env: &Env,
    field: ConfigField,
    previous: bool,
    new: bool,
    admin: &Address,
    timestamp: u64,
) -> bool {
    if previous == new {
        return false;
    }
    emit_config_updated(
        env,
        field,
        ConfigValue::Bool(previous),
        ConfigValue::Bool(new),
        admin,
        timestamp,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, String,
    };

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }
    #[ignore]
    #[test]
    fn emit_config_updated_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            emit_config_updated(
                &env,
                ConfigField::PlatformFee,
                ConfigValue::Number(100),
                ConfigValue::Number(250),
                &admin,
                1_700_000_000,
            );
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
    #[ignore]
    #[test]
    fn numeric_change_emits_when_value_differs() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            assert!(emit_numeric_change(
                &env,
                ConfigField::MaxRoyalty,
                500,
                1_000,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
    #[ignore]
    #[test]
    fn numeric_change_is_silent_when_value_is_unchanged() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            assert!(!emit_numeric_change(
                &env,
                ConfigField::BatchSize,
                50,
                50,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
    #[ignore]
    #[test]
    fn address_change_emits_when_value_differs() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let old_asset = Address::generate(&env);
            let new_asset = Address::generate(&env);
            assert!(emit_address_change(
                &env,
                ConfigField::SupportedAsset,
                &old_asset,
                &new_asset,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
    #[ignore]
    #[test]
    fn address_change_is_silent_when_value_is_unchanged() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let asset = Address::generate(&env);
            assert!(!emit_address_change(
                &env,
                ConfigField::SupportedAsset,
                &asset,
                &asset,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 0);
        });
    }

    #[test]
    fn string_change_emits_when_value_differs_and_skips_when_unchanged() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let old_uri = String::from_str(&env, "https://old.example.com/");
            let new_uri = String::from_str(&env, "https://new.example.com/");

            assert!(!emit_string_change(
                &env,
                ConfigField::MetadataBaseUri,
                &old_uri,
                &old_uri,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 0);

            assert!(emit_string_change(
                &env,
                ConfigField::MetadataBaseUri,
                &old_uri,
                &new_uri,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn bool_change_emits_when_value_differs_and_skips_when_unchanged() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);

            assert!(!emit_bool_change(
                &env,
                ConfigField::Paused,
                false,
                false,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 0);

            assert!(emit_bool_change(
                &env,
                ConfigField::Paused,
                false,
                true,
                &admin,
                1_700_000_000,
            ));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
}
