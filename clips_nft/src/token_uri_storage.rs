//! Token URI storage — saves and retrieves validated metadata URIs for every NFT.
//!
//! # Storage
//! Key: `DataKey::TokenUri(token_id)` (persistent storage)

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Validate a metadata URI — rejects empty strings.
pub fn validate_uri(uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    Ok(())
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
