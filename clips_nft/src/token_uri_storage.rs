//! Token URI storage — saves and retrieves validated metadata URIs for every NFT.
//!
//! # Storage
//! Key: `DataKey::TokenUri(token_id)` (persistent storage)

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Validate a metadata URI — rejects empty strings and invalid formats.
/// Supported schemes: `ipfs://`, `https://`, `ar://`
pub fn validate_uri(uri: &String) -> Result<(), Error> {
    crate::metadata_uri_builder::validate_uri(uri)
}

/// Save a metadata URI for a token.
///
/// Validates the URI before persisting. Returns `Err(InvalidURI)` for an empty string.
pub fn set_token_uri(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    validate_uri(uri)?;
    env.storage()
        .persistent()
        .set(&DataKey::TokenUri(token_id), uri);
    Ok(())
}

/// Read the metadata URI for a token.
///
/// Returns `Err(TokenNotFound)` if no URI has been stored.
pub fn get_token_uri(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenUri(token_id))
        .ok_or(Error::TokenNotFound)
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
    fn set_and_get_token_uri() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://clip/1");
            set_token_uri(env, 1, &uri).unwrap();
            assert_eq!(get_token_uri(env, 1).unwrap(), uri);
        });
    }

    #[test]
    fn get_token_uri_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_token_uri(env, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn set_token_uri_rejects_empty_uri() {
        with_contract(|env| {
            let uri = String::from_str(env, "");
            assert_eq!(set_token_uri(env, 1, &uri), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn set_token_uri_rejects_unsupported_scheme() {
        with_contract(|env| {
            let uri = String::from_str(env, "ftp://clip/1");
            assert_eq!(set_token_uri(env, 1, &uri), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn validate_uri_accepts_supported_schemes() {
        let env = Env::default();
        assert!(validate_uri(&String::from_str(&env, "ipfs://cid")).is_ok());
        assert!(validate_uri(&String::from_str(&env, "https://x/y")).is_ok());
        assert!(validate_uri(&String::from_str(&env, "ar://id")).is_ok());
    }
}
