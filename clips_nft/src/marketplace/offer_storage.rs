//! Offer storage — persistence layer for marketplace buy offers.
//!
//! # Storage
//! Key: `DataKey::Offer(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::{DataKey, Error, TokenId};

use super::types::Offer;

/// Save an offer. Overwrites any existing offer for the same token.
pub fn save_offer(env: &Env, offer: &Offer) {
    env.storage()
        .persistent()
        .set(&DataKey::Offer(offer.token_id), offer);
}

/// Load an offer by token ID. Returns `Err(TokenNotFound)` if absent.
pub fn get_offer(env: &Env, token_id: TokenId) -> Result<Offer, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Offer(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Check whether an offer exists for the given token.
pub fn has_offer(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Offer(token_id))
}

/// Remove an offer from storage.
pub fn remove_offer(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Offer(token_id));
}
