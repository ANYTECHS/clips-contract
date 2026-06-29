//! Token metadata storage — save, retrieve, and update NFT metadata URIs.
//!
//! # Storage
//! Key: `DataKey::Metadata(token_id)` (persistent storage)

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Persist the metadata URI for `token_id`.
pub fn save_metadata(env: &Env, token_id: TokenId, uri: &String) {
    env.storage().persistent().set(&DataKey::Metadata(token_id), uri);
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
    env.storage().persistent().set(&DataKey::Metadata(token_id), uri);
    Ok(())
}
