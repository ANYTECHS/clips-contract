//! Supported payment currency configuration (Issue #480).
//!
//! Allows administrators to define accepted payment currencies.
//! Provides storage, deduplication, getter, and admin update.

use soroban_sdk::{Address, Env, Vec};

use crate::config_updated_event;
use crate::types::{ConfigField, ConfigValue, DataKey, Error};

/// Add a payment currency to the supported list.
///
/// Prevents duplicates — returns [`Error::DuplicateCurrency`] if already present.
pub fn add_currency(env: &Env, currency: Address) -> Result<(), Error> {
    let mut currencies = get_currencies(env);

    // Check for duplicates
    for i in 0..currencies.len() {
        if let Some(c) = currencies.get(i) {
            if c == currency {
                return Err(Error::DuplicateCurrency);
            }
        }
    }

    currencies.push_back(currency);
    env.storage()
        .instance()
        .set(&DataKey::SupportedCurrencies, &currencies);
    Ok(())
}

/// Remove a payment currency from the supported list.
///
/// Returns [`Error::CurrencyNotFound`] if the currency is not in the list.
pub fn remove_currency(env: &Env, currency: &Address) -> Result<(), Error> {
    let currencies = get_currencies(env);
    let mut new_list = Vec::new(env);
    let mut found = false;

    for i in 0..currencies.len() {
        if let Some(c) = currencies.get(i) {
            if c == *currency {
                found = true;
            } else {
                new_list.push_back(c);
            }
        }
    }

    if !found {
        return Err(Error::CurrencyNotFound);
    }

    env.storage()
        .instance()
        .set(&DataKey::SupportedCurrencies, &new_list);
    Ok(())
}

/// Get the list of supported payment currencies.
pub fn get_currencies(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::SupportedCurrencies)
        .unwrap_or_else(|| Vec::new(env))
}

/// Check if a currency is in the supported list.
pub fn is_supported(env: &Env, currency: &Address) -> bool {
    let currencies = get_currencies(env);
    for i in 0..currencies.len() {
        if let Some(c) = currencies.get(i) {
            if c == *currency {
                return true;
            }
        }
    }
    false
}

/// Add a supported payment currency on behalf of `admin`, emitting a
/// [`ConfigUpdatedEvent`] on success (issue #932).
///
/// Thin wrapper over [`add_currency`] for callers that have an authenticated
/// administrator to attribute the change to. The event records
/// [`ConfigValue::Unset`] as the previous value, since the asset was not
/// previously supported.
///
/// [`ConfigUpdatedEvent`]: crate::types::ConfigUpdatedEvent
/// [`ConfigValue::Unset`]: crate::types::ConfigValue::Unset
pub fn add_currency_by(env: &Env, admin: &Address, currency: Address) -> Result<(), Error> {
    add_currency(env, currency.clone())?;
    config_updated_event::emit_config_updated(
        env,
        ConfigField::SupportedAsset,
        ConfigValue::Unset,
        ConfigValue::Address(currency),
        admin,
        env.ledger().timestamp(),
    );
    Ok(())
}

/// Remove a supported payment currency on behalf of `admin`, emitting a
/// [`ConfigUpdatedEvent`] on success (issue #932).
///
/// The event records [`ConfigValue::Unset`] as the new value, since the asset
/// is no longer supported after the call.
///
/// [`ConfigUpdatedEvent`]: crate::types::ConfigUpdatedEvent
/// [`ConfigValue::Unset`]: crate::types::ConfigValue::Unset
pub fn remove_currency_by(env: &Env, admin: &Address, currency: &Address) -> Result<(), Error> {
    remove_currency(env, currency)?;
    config_updated_event::emit_config_updated(
        env,
        ConfigField::SupportedAsset,
        ConfigValue::Address(currency.clone()),
        ConfigValue::Unset,
        admin,
        env.ledger().timestamp(),
    );
    Ok(())
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
    fn add_currency_by_registers_and_emits() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let asset = Address::generate(&env);

            assert!(add_currency_by(&env, &admin, asset.clone()).is_ok());
            assert!(is_supported(&env, &asset));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn add_currency_by_emits_nothing_on_duplicate() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let asset = Address::generate(&env);
            add_currency(&env, asset.clone()).unwrap();

            assert_eq!(
                add_currency_by(&env, &admin, asset),
                Err(Error::DuplicateCurrency)
            );
            assert_eq!(env.events().all().events().len(), 0);
        });
    }

    #[test]
    fn remove_currency_by_deregisters_and_emits() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let asset = Address::generate(&env);
            add_currency(&env, asset.clone()).unwrap();

            assert!(remove_currency_by(&env, &admin, &asset).is_ok());
            assert!(!is_supported(&env, &asset));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn remove_currency_by_emits_nothing_when_absent() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let asset = Address::generate(&env);

            assert_eq!(
                remove_currency_by(&env, &admin, &asset),
                Err(Error::CurrencyNotFound)
            );
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
