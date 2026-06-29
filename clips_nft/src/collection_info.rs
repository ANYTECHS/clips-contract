//! Collection name and symbol storage.
//!
//! Stores the collection's human-readable name and symbol.
//!
//! # Storage
//! - Name: `DataKey::Name` (instance storage)
//! - Symbol: `DataKey::Symbol` (instance storage)

use soroban_sdk::{Env, String};

use crate::types::DataKey;

/// Default collection name.
pub const DEFAULT_NAME: &str = "ClipCash Clips";
/// Default collection symbol.
pub const DEFAULT_SYMBOL: &str = "CLIP";

/// Store the collection name.
pub fn set_name(env: &Env, name: &String) {
    env.storage().instance().set(&DataKey::Name, name);
}

/// Return the collection name.
///
/// Falls back to [`DEFAULT_NAME`] if never explicitly set.
pub fn get_name(env: &Env) -> String {
    env.storage()
        .instance()
        .get(&DataKey::Name)
        .unwrap_or_else(|| String::from_str(env, DEFAULT_NAME))
}

/// Store the collection symbol.
pub fn set_symbol(env: &Env, symbol: &String) {
    env.storage().instance().set(&DataKey::Symbol, symbol);
}

/// Return the collection symbol.
///
/// Falls back to [`DEFAULT_SYMBOL`] if never explicitly set.
pub fn get_symbol(env: &Env) -> String {
    env.storage()
        .instance()
        .get(&DataKey::Symbol)
        .unwrap_or_else(|| String::from_str(env, DEFAULT_SYMBOL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn get_default_name_and_symbol() {
        let env = Env::default();
        assert_eq!(get_name(&env), String::from_str(&env, DEFAULT_NAME));
        assert_eq!(get_symbol(&env), String::from_str(&env, DEFAULT_SYMBOL));
    }

    #[test]
    fn set_and_get_name_and_symbol() {
        let env = Env::default();
        let new_name = String::from_str(&env, "Test Collection");
        let new_symbol = String::from_str(&env, "TEST");

        set_name(&env, &new_name);
        set_symbol(&env, &new_symbol);

        assert_eq!(get_name(&env), new_name);
        assert_eq!(get_symbol(&env), new_symbol);
    }
}
