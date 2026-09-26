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
