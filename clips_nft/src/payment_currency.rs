//! Supported payment currency configuration (Issue #480).
//!
//! Allows administrators to define accepted payment currencies.
//! Provides storage, deduplication, getter, and admin update.

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, Error};

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
