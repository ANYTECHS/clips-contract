//! Royalty storage — save, retrieve, and update per-token royalty configuration.
//!
//! # Storage
//! Key: `DataKey::Royalty(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::royalty_updated_event;
use crate::types::{DataKey, Error, Royalty, TokenId};

/// Persist the royalty configuration for `token_id`.
pub fn save_royalty(env: &Env, token_id: TokenId, royalty: &Royalty) {
    env.storage()
        .persistent()
        .set(&DataKey::Royalty(token_id), royalty);
}

/// Load the royalty configuration for `token_id`. Returns `Err(TokenNotFound)` if absent.
pub fn get_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Royalty(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Overwrite the royalty configuration for `token_id`. Returns `Err(TokenNotFound)` if the
/// token does not exist.
///
/// Optimized to use a single storage read to verify token existence before writing.
/// Emits a [`RoyaltyUpdatedEvent`] after successful update.
pub fn update_royalty(env: &Env, token_id: TokenId, royalty: &Royalty) -> Result<(), Error> {
    if env.storage().persistent().has(&DataKey::RoyaltyFrozen(token_id)) {
        return Err(Error::RoyaltyFrozen);
    }
    if !env.storage().persistent().has(&DataKey::Royalty(token_id)) {
    if !env.storage().persistent().has(&DataKey::Token(token_id)) {
        return Err(Error::TokenNotFound);
    }
    env.storage()
        .persistent()
        .set(&DataKey::Royalty(token_id), royalty);
    royalty_updated_event::emit_royalty_updated(env, token_id, royalty, env.ledger().timestamp());
    Ok(())
}

/// Permanently freeze the royalty configuration for `token_id`.
pub fn freeze_royalty(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Royalty(token_id)) {
        return Err(Error::TokenNotFound);
    }
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyFrozen(token_id), &true);
    Ok(())
}
