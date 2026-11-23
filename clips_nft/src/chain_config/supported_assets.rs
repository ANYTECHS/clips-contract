//! Supported Asset Configuration — resolves issue #1140.
//!
//! Maintains a list of Stellar assets supported by the contract.

use soroban_sdk::{Address, Env, Vec};

use crate::types::Error;
use super::keys::ConfigKey;

/// Retrieve the list of supported Stellar assets.
pub fn get_supported_assets(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&ConfigKey::SupportedAssets)
        .unwrap_or_else(|| Vec::new(env))
}

/// Check if a Stellar asset is supported.
pub fn is_asset_supported(env: &Env, asset: &Address) -> bool {
    let assets = get_supported_assets(env);
    for i in 0..assets.len() {
        if let Some(a) = assets.get(i) {
            if a == *asset {
                return true;
            }
        }
    }
    false
}

/// Add a Stellar asset to the supported assets list.
pub fn add_supported_asset(env: &Env, asset: Address) -> Result<(), Error> {
    let mut assets = get_supported_assets(env);
    for i in 0..assets.len() {
        if let Some(a) = assets.get(i) {
            if a == asset {
                return Err(Error::DuplicateCurrency);
            }
        }
    }
    assets.push_back(asset);
    env.storage()
        .instance()
        .set(&ConfigKey::SupportedAssets, &assets);
    Ok(())
}

/// Remove a Stellar asset from the supported assets list.
pub fn remove_supported_asset(env: &Env, asset: &Address) -> Result<(), Error> {
    let assets = get_supported_assets(env);
    let mut updated = Vec::new(env);
    let mut found = false;
    for i in 0..assets.len() {
        if let Some(a) = assets.get(i) {
            if a == *asset {
                found = true;
            } else {
                updated.push_back(a);
            }
        }
    }
    if !found {
        return Err(Error::CurrencyNotFound);
    }
    env.storage()
        .instance()
        .set(&ConfigKey::SupportedAssets, &updated);
    Ok(())
}
