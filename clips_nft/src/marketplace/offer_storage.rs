//! Persistent storage for marketplace offers.

use soroban_sdk::Env;

use super::types::Offer;
use crate::types::{DataKey, Error, TokenId};

pub fn save_offer(env: &Env, offer: &Offer) {
    env.storage()
        .persistent()
        .set(&DataKey::Offer(offer.token_id), offer);
}

pub fn create_offer(env: &Env, offer: &Offer) -> Result<(), Error> {
    if has_offer(env, offer.token_id) {
        return Err(Error::OfferAlreadyExists);
    }
    save_offer(env, offer);
    Ok(())
}

pub fn get_offer(env: &Env, token_id: TokenId) -> Result<Offer, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Offer(token_id))
        .ok_or(Error::OfferExpired)
}

pub fn has_offer(env: &Env, token_id: TokenId) -> bool {
    env.storage().persistent().has(&DataKey::Offer(token_id))
}

pub fn update_offer(env: &Env, offer: &Offer) -> Result<(), Error> {
    if !has_offer(env, offer.token_id) {
        return Err(Error::OfferExpired);
    }
    save_offer(env, offer);
    Ok(())
}

pub fn update_offer_status(
    env: &Env,
    token_id: TokenId,
    status: super::types::OfferStatus,
) -> Result<(), Error> {
    let mut offer = get_offer(env, token_id)?;
    offer.status = status;
    update_offer(env, &offer)
}

pub fn remove_offer(env: &Env, token_id: TokenId) {
    env.storage().persistent().remove(&DataKey::Offer(token_id));
}
