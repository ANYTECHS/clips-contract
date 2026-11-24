//! Supported Payment Currency Configuration — resolves issue #1141.
//!
//! Allows administrators to define accepted payment currencies with deduplication and queries.

use soroban_sdk::{Address, Env, Vec};

use crate::types::Error;
use super::keys::ConfigKey;

/// Retrieve all accepted payment currencies.
pub fn get_payment_currencies(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&ConfigKey::SupportedCurrencies)
        .unwrap_or_else(|| Vec::new(env))
}

/// Check whether a payment currency is supported.
pub fn is_currency_supported(env: &Env, currency: &Address) -> bool {
    let currencies = get_payment_currencies(env);
    for i in 0..currencies.len() {
        if let Some(c) = currencies.get(i) {
            if c == *currency {
                return true;
            }
        }
    }
    false
}

/// Add an accepted payment currency, rejecting duplicates.
pub fn add_payment_currency(env: &Env, currency: Address) -> Result<(), Error> {
    let mut currencies = get_payment_currencies(env);
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
        .set(&ConfigKey::SupportedCurrencies, &currencies);
    Ok(())
}

/// Remove a payment currency from the accepted list.
pub fn remove_payment_currency(env: &Env, currency: &Address) -> Result<(), Error> {
    let currencies = get_payment_currencies(env);
    let mut updated = Vec::new(env);
    let mut found = false;
    for i in 0..currencies.len() {
        if let Some(c) = currencies.get(i) {
            if c == *currency {
                found = true;
            } else {
                updated.push_back(c);
            }
        }
    }
    if !found {
        return Err(Error::CurrencyNotFound);
    }
    env.storage()
        .instance()
        .set(&ConfigKey::SupportedCurrencies, &updated);
    Ok(())
}
