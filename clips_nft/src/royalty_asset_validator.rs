//! Royalty payment asset validator (issue #810).
//!
//! Ensures that royalty payments use an asset supported by the contract.
//! Supports Stellar native (XLM) when `asset_address` is `None`, and
//! SEP-0041 tokens when `asset_address` is `Some(address)` and the token
//! is registered in the supported currencies list.

use soroban_sdk::{Address, Env};

use crate::payment_currency;
use crate::types::Error;

/// Validate that the given asset address is supported for royalty payments.
///
/// - `None` → valid (Stellar native / XLM).
/// - `Some(addr)` → must be registered via [`payment_currency::add_currency`].
///
/// # Errors
/// - [`Error::UnsupportedAsset`] if the token is not in the supported list.
pub fn validate_royalty_asset(
    env: &Env,
    asset_address: &Option<Address>,
) -> Result<(), Error> {
    match asset_address {
        None => Ok(()),
        Some(addr) => {
            if payment_currency::is_supported(env, addr) {
                Ok(())
            } else {
                Err(Error::UnsupportedAsset)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn native_asset_is_valid() {
        with_contract(|env| {
            assert!(validate_royalty_asset(env, &None).is_ok());
        });
    }

    #[test]
    fn supported_token_is_valid() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&crate::types::DataKey::Admin, &admin);

            let token = Address::generate(env);
            payment_currency::add_currency(env, token.clone()).unwrap();
            assert!(validate_royalty_asset(env, &Some(token)).is_ok());
        });
    }

    #[test]
    fn unsupported_token_rejected() {
        with_contract(|env| {
            let token = Address::generate(env);
            assert_eq!(
                validate_royalty_asset(env, &Some(token)),
                Err(Error::UnsupportedAsset)
            );
        });
    }
}
