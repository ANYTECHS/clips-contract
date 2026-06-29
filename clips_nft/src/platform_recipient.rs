//! Platform recipient (treasury wallet) storage.
//!
//! Stores the wallet address responsible for receiving platform fees.
//!
//! # Storage
//! Key: `DataKey::PlatformRecipient` (instance storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error};

/// Save the treasury wallet address.
///
/// Creates or overwrites the stored address.
pub fn save_platform_recipient(env: &Env, recipient: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::PlatformRecipient, recipient);
}

/// Retrieve the treasury wallet address.
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no treasury wallet has been saved.
pub fn get_platform_recipient(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::PlatformRecipient)
        .ok_or(Error::NotInitialized)
}

/// Update the treasury wallet address.
///
/// # Errors
/// Returns [`Error::NotInitialized`] if no treasury wallet has been saved yet.
pub fn update_platform_recipient(env: &Env, recipient: &Address) -> Result<(), Error> {
    if !env
        .storage()
        .instance()
        .has(&DataKey::PlatformRecipient)
    {
        return Err(Error::NotInitialized);
    }
    env.storage()
        .instance()
        .set(&DataKey::PlatformRecipient, recipient);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn save_and_retrieve_treasury_wallet() {
        let env = Env::default();
        let wallet = Address::generate(&env);

        save_platform_recipient(&env, &wallet);
        assert_eq!(get_platform_recipient(&env).unwrap(), wallet);
    }

    #[test]
    fn retrieve_before_save_returns_not_initialized() {
        let env = Env::default();
        assert_eq!(
            get_platform_recipient(&env),
            Err(Error::NotInitialized)
        );
    }

    #[test]
    fn update_treasury_wallet() {
        let env = Env::default();
        let initial = Address::generate(&env);
        let updated = Address::generate(&env);

        save_platform_recipient(&env, &initial);
        update_platform_recipient(&env, &updated).unwrap();
        assert_eq!(get_platform_recipient(&env).unwrap(), updated);
    }

    #[test]
    fn update_before_save_returns_not_initialized() {
        let env = Env::default();
        let wallet = Address::generate(&env);
        assert_eq!(
            update_platform_recipient(&env, &wallet),
            Err(Error::NotInitialized)
        );
    }

    #[test]
    fn save_overwrites_existing_wallet() {
        let env = Env::default();
        let first = Address::generate(&env);
        let second = Address::generate(&env);

        save_platform_recipient(&env, &first);
        save_platform_recipient(&env, &second);
        assert_eq!(get_platform_recipient(&env).unwrap(), second);
    }
}
