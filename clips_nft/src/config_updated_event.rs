//! Contract configuration-updated event (issue #932).
//!
//! Emitted whenever an administrator changes a contract-wide setting — platform
//! fee, maximum royalty, batch size, supported asset, or any other contract
//! limit. Each event carries the previous and the new value so an indexer can
//! reconstruct the full configuration history without replaying storage.
//!
//! # Event topic
//! `"cfg_updt"` — within the 9-character limit for [`soroban_sdk::symbol_short`].

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{ConfigField, ConfigUpdatedEvent, ConfigValue};

/// Emit the `"cfg_updt"` event after a configuration value has been persisted.
///
/// Call once per changed setting; a call that updates several settings at once
/// emits one event per field that actually changed, so subscribers never have
/// to diff values themselves.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `field`          — Setting that changed.
/// * `previous_value` — Value before the update.
/// * `new_value`      — Value after the update.
/// * `admin`          — Account that performed the update.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

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
}
