//! Creator verification guard (issue #1412).
//!
//! Retrieves the registered creator of an NFT, compares it with the caller,
//! and rejects unauthorized callers with errors from the centralized
//! [`crate::types::Error`] catalog. This module exposes the existing creator
//! guard through the centralized guards namespace.

use crate::creator_guard;
use crate::creator_storage;
use crate::types::{Error, TokenId};
use soroban_sdk::{Address, Env};

/// Retrieve the registered creator for `token_id` (#1412).
pub fn get_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    creator_storage::get_creator(env, token_id)
}

/// Require that `caller` is the registered creator of `token_id` (#1412).
///
/// Also enforces a signed authorization envelope via `require_auth`.
pub fn require_creator(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    creator_guard::require_creator(env, caller, token_id)
}

/// Non-panicking probe: is `caller` the registered creator?
pub fn is_creator(env: &Env, caller: &Address, token_id: TokenId) -> bool {
    matches!(get_creator(env, token_id), Ok(creator) if creator == *caller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::testutils::Address as _;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn retrieves_registered_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            crate::creator_storage::set_creator(env, 7, &creator);
            assert_eq!(get_creator(env, 7), Ok(creator));
        });
    }

    #[test]
    fn creator_is_accepted_and_non_creator_rejected() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let stranger = Address::generate(env);
            crate::creator_storage::set_creator(env, 7, &creator);

            assert!(is_creator(env, &creator, 7));
            assert!(!is_creator(env, &stranger, 7));
            assert_eq!(require_creator(env, &creator, 7), Ok(()));
            assert_eq!(
                require_creator(env, &stranger, 7),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn missing_creator_record_is_rejected() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(!is_creator(env, &caller, 99));
            assert_eq!(
                require_creator(env, &caller, 99),
                Err(Error::TokenNotFound)
            );
        });
    }
}
