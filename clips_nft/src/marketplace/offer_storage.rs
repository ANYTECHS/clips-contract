//! Marketplace offer storage — persistent CRUD for [`Offer`] records.

use soroban_sdk::Env;

use crate::marketplace::types::Offer;
use crate::types::{DataKey, Error, TokenId};

/// Persist `offer` under `DataKey::Offer(offer.token_id)`.
pub fn save_offer(env: &Env, offer: &Offer) {
    env.storage()
        .persistent()
        .set(&DataKey::Offer(offer.token_id), offer);
}

/// Return the [`Offer`] for `token_id`.
pub fn get_offer(env: &Env, token_id: TokenId) -> Result<Offer, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Offer(token_id))
        .ok_or(Error::ListingNotFound)
}

/// Return `true` if an offer exists for `token_id`.
pub fn has_offer(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Offer(token_id))
}

/// Update an existing offer in-place.
pub fn update_offer(env: &Env, offer: &Offer) -> Result<(), Error> {
    if !has_offer(env, offer.token_id) {
        return Err(Error::ListingNotFound);
    }
    save_offer(env, offer);
    Ok(())
}

/// Remove the offer record for `token_id`.
pub fn remove_offer(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Offer(token_id));
}
