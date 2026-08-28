//! Offer storage — persistence layer for marketplace buy offers.
//!
//! # Storage
//! Key: `DataKey::Offer(token_id)` (persistent storage) holds the offer.
//! Key: `DataKey::OfferIndex` (instance storage) holds a `Vec<TokenId>` index
//! of every token that currently has an open offer, used to enumerate and prune
//! expired offers without relying on storage key iteration.

use soroban_sdk::{Env, Vec};

use crate::types::{DataKey, Error, TokenId};

use super::types::{Offer, OfferStatus};

/// Load the offer index (empty if none).
fn load_index(env: &Env) -> Vec<TokenId> {
    env.storage()
        .instance()
        .get(&DataKey::OfferIndex)
        .unwrap_or_else(|| Vec::new(env))
}

/// Persist the offer index.
fn save_index(env: &Env, index: &Vec<TokenId>) {
    env.storage().instance().set(&DataKey::OfferIndex, index);
}

/// Add a token ID to the offer index (deduplicated).
fn index_add(env: &Env, token_id: TokenId) {
    let mut index = load_index(env);
    for i in 0..index.len() {
        if index.get(i).unwrap() == token_id {
            return;
        }
    }
    index.push_back(token_id);
    save_index(env, &index);
}

/// Remove a token ID from the offer index, if present.
fn index_remove(env: &Env, token_id: TokenId) {
    let index = load_index(env);
    let mut next: Vec<TokenId> = Vec::new(env);
    for i in 0..index.len() {
        let id = index.get(i).unwrap();
        if id != token_id {
            next.push_back(id);
        }
    }
    save_index(env, &next);
}

/// Save an offer. Overwrites any existing offer for the same token and keeps
/// the offer index in sync.
pub fn save_offer(env: &Env, offer: &Offer) {
    env.storage()
        .persistent()
        .set(&DataKey::Offer(offer.token_id), offer);
    index_add(env, offer.token_id);
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

/// Update an existing offer in place (#886).
pub fn update_offer(env: &Env, offer: &Offer) -> Result<(), Error> {
    if !has_offer(env, offer.token_id) {
        return Err(Error::TokenNotFound);
    }
    save_offer(env, offer);
    Ok(())
}

/// Remove an offer from storage.
pub fn remove_offer(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Offer(token_id));
    index_remove(env, token_id);
}

/// Remove all expired offers. Returns the number of offers removed (#886).
pub fn remove_expired_offers(env: &Env) -> u32 {
    let now = env.ledger().timestamp();
    let index = load_index(env);
    let mut removed: u32 = 0;

    for i in 0..index.len() {
        let token_id = index.get(i).unwrap();
        if let Some(offer) = env
            .storage()
            .persistent()
            .get::<DataKey, Offer>(&DataKey::Offer(token_id))
        {
            if offer.expires_at > 0 && offer.expires_at <= now {
                remove_offer(env, token_id);
                removed += 1;
            }
        }
    }

    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::types::{Offer, OfferStatus};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Address;

    fn sample_offer(token_id: TokenId, buyer: &Address, expires_at: u64) -> Offer {
        Offer {
            offer_id: 1,
            token_id,
            buyer: buyer.clone(),
            price: 5_000,
            payment_asset: Address::generate(&Env::default()),
            expires_at,
            status: OfferStatus::Active,
            created_at: 0,
        }
    }

    #[test]
    fn save_and_get_offer() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let offer = sample_offer(1, &buyer, 0);
        save_offer(&env, &offer);
        let loaded = get_offer(&env, 1).unwrap();
        assert_eq!(loaded.buyer, buyer);
        assert_eq!(loaded.price, 5_000);
        assert_eq!(loaded.status, OfferStatus::Active);
    }

    #[test]
    fn has_offer_works() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        assert!(!has_offer(&env, 1));
        save_offer(&env, &sample_offer(1, &buyer, 0));
        assert!(has_offer(&env, 1));
    }

    #[test]
    fn update_offer_works() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let mut offer = sample_offer(1, &buyer, 0);
        save_offer(&env, &offer);

        offer.price = 10_000;
        offer.status = OfferStatus::Accepted;
        update_offer(&env, &offer).unwrap();

        let loaded = get_offer(&env, 1).unwrap();
        assert_eq!(loaded.price, 10_000);
        assert_eq!(loaded.status, OfferStatus::Accepted);
    }

    #[test]
    fn update_offer_not_found() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let offer = sample_offer(999, &buyer, 0);
        assert_eq!(update_offer(&env, &offer), Err(Error::TokenNotFound));
    }

    #[test]
    fn remove_offer_works() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        save_offer(&env, &sample_offer(1, &buyer, 0));
        assert!(has_offer(&env, 1));
        remove_offer(&env, 1);
        assert!(!has_offer(&env, 1));
    }

    #[test]
    fn remove_expired_offers_removes_old() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        // expires_at = 100 is in the past relative to default ledger timestamp.
        save_offer(&env, &sample_offer(1, &buyer, 100));
        // expires_at = 0 means no expiration — should not be removed.
        save_offer(&env, &sample_offer(2, &buyer, 0));

        let removed = remove_expired_offers(&env);
        assert_eq!(removed, 1);
        assert!(!has_offer(&env, 1));
        assert!(has_offer(&env, 2));
    }
}
