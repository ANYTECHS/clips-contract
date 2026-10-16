//! Open-offer persistence, keyed by token.
//!
//! One open offer per token (`DataKey::Offer(token_id)`). Creating another
//! fails with `Error::OfferAlreadyExists`; accepting marks the record
//! `Accepted` and then removes it so the token can receive fresh offers.

use soroban_sdk::Env;

use super::types::Offer;
use crate::types::{DataKey, Error, TokenId};

/// True when any open offer record exists for the token.
pub fn has_offer(env: &Env, token_id: TokenId) -> bool {
    env.storage().persistent().has(&DataKey::Offer(token_id))
}

/// Persist a new offer. Callers check [`has_offer`] first.
pub fn save_offer(env: &Env, offer: &Offer) {
    env.storage()
        .persistent()
        .set(&DataKey::Offer(offer.token_id), offer);
}

/// Load the open offer for a token.
pub fn get_offer(env: &Env, token_id: TokenId) -> Result<Offer, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Offer(token_id))
        .ok_or(Error::OfferExpired)
}

/// Persist an updated offer (e.g. marking it `Accepted`).
pub fn update_offer(env: &Env, offer: &Offer) -> Result<(), Error> {
    let key = DataKey::Offer(offer.token_id);
    if !env.storage().persistent().has(&key) {
        return Err(Error::OfferExpired);
    }
    env.storage().persistent().set(&key, offer);
    Ok(())
}

/// Remove the offer record (accepted or cancelled).
pub fn remove_offer(env: &Env, token_id: TokenId) {
    env.storage().persistent().remove(&DataKey::Offer(token_id));
}
