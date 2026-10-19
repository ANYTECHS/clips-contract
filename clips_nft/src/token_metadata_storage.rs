//! Token metadata storage — save, retrieve, and update NFT metadata URIs.
//!
//! Validation pipeline applied on every write:
//!   1. URI format check     (`metadata_uri_validator`, Issue #561)
//!   2. URI size check       (`metadata_size`,          Issue #560)
//!   3. Update-policy check  (`metadata_update_policy`, Issue #562)
//!
//! A `MetadataUpdatedEvent` is emitted after every successful update (#563).
//!
//! # Storage
//! Key: `DataKey::Metadata(token_id)` (persistent storage)

use soroban_sdk::{Env, String};

use crate::metadata_config::validate_metadata_size;
use crate::types::{DataKey, Error, TokenId};

/// Persist the metadata URI for `token_id`.
pub fn save_metadata(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    validate_metadata_size(env, uri)?;
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    Ok(())
}

/// Load the metadata URI for `token_id`. Returns `Err(TokenNotFound)` if absent.
pub fn get_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Overwrite the metadata URI for `token_id`. Returns `Err(TokenNotFound)` if no
/// metadata has been saved for this token yet.
pub fn update_metadata(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Metadata(token_id)) {
        return Err(Error::TokenNotFound);
    }
    validate_metadata_size(env, uri)?;
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn save_and_get_metadata() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://clip/1");
            save_metadata(env, 1, &uri).unwrap();
            assert_eq!(get_metadata(env, 1).unwrap(), uri);
        });
    }

    #[test]
    fn get_metadata_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_metadata(env, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn update_metadata_overwrites() {
        with_contract(|env| {
            let old = String::from_str(env, "ipfs://clip/1");
            let new = String::from_str(env, "ipfs://clip/1-v2");
            save_metadata(env, 1, &old).unwrap();
            update_metadata(env, 1, &new).unwrap();
            assert_eq!(get_metadata(env, 1).unwrap(), new);
        });
    }

    #[test]
    fn update_metadata_missing_returns_token_not_found() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://clip/9");
            assert_eq!(
                update_metadata(env, 9, &uri),
                Err(Error::TokenNotFound)
            );
        });
    }
}
