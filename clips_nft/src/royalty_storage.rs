//! Royalty storage — save, retrieve, and update per-token royalty configuration.
//!
//! # Storage
//! Key: `DataKey::Royalty(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::{DataKey, Error, Royalty, TokenId};

/// Persist the royalty configuration for `token_id`.
pub fn save_royalty(env: &Env, token_id: TokenId, royalty: &Royalty) {
    env.storage().persistent().set(&DataKey::Royalty(token_id), royalty);
}

/// Load the royalty configuration for `token_id`. Returns `Err(TokenNotFound)` if absent.
pub fn get_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Royalty(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Overwrite the royalty configuration for `token_id`. Returns `Err(TokenNotFound)` if no
/// royalty has been saved for this token yet.
pub fn update_royalty(env: &Env, token_id: TokenId, royalty: &Royalty) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Royalty(token_id)) {
        return Err(Error::TokenNotFound);
    }
    env.storage().persistent().set(&DataKey::Royalty(token_id), royalty);
    Ok(())
}
