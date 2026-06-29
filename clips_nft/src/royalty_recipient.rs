//! Royalty recipient storage.
//!
//! Stores royalty recipient addresses separately from the full `Royalty`
//! struct, allowing lightweight recipient lookups and updates.
//!
//! # Storage
//! Key: `DataKey::RoyaltyRecipient(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

/// Persist the royalty recipient for a given token.
///
/// Creates or overwrites the stored address, acting as both save and update.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if the token does not exist.
pub fn set_royalty_recipient(
    env: &Env,
    token_id: TokenId,
    recipient: Address,
) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Token(token_id)) {
        return Err(Error::TokenNotFound);
    }
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyRecipient(token_id), &recipient);
    Ok(())
}

/// Return the royalty recipient for a given token, or `None` if not set.
pub fn get_royalty_recipient(env: &Env, token_id: TokenId) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::RoyaltyRecipient(token_id))
}
