//! Offer storage — persistence layer for marketplace buy offers.
//!
//! # Storage
//! Key: `DataKey::Offer(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::{
    offer_accepted_event, offer_created_event, token_storage, types::{DataKey, Error, TokenId},
};

use super::types::{Offer, OfferStatus};

/// Save an offer. Overwrites any existing offer for the same token.
///
/// Emits an [`OfferCreatedEvent`](crate::offer_created_event) the first time an
/// offer is persisted for a token (issue #926).
pub fn save_offer(env: &Env, offer: &Offer) {
    let is_new = !has_offer(env, offer.token_id);
    env.storage()
        .persistent()
        .set(&DataKey::Offer(offer.token_id), offer);
    if is_new {
        offer_created_event::emit_offer_created(
            env,
            offer.offer_id,
            offer.token_id,
            &offer.buyer,
            offer.price,
            &offer.payment_asset,
            offer.expires_at,
        );
    }
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
///
/// When the updated offer transitions to `Accepted`, emits an
/// [`OfferAcceptedEvent`](crate::offer_accepted_event) (issue #927). The seller
/// is resolved to the token's current owner at acceptance time.
pub fn update_offer(env: &Env, offer: &Offer) -> Result<(), Error> {
    if !has_offer(env, offer.token_id) {
        return Err(Error::TokenNotFound);
    }
    save_offer(env, offer);
    if offer.status == OfferStatus::Accepted {
        let seller = token_storage::get_token(env, offer.token_id)
            .map(|data| data.owner)
            .unwrap_or_else(|_| offer.buyer.clone());
        offer_accepted_event::emit_offer_accepted(
            env,
            offer.offer_id,
            offer.token_id,
            &offer.buyer,
            &seller,
            offer.price,
            env.ledger().timestamp(),
        );
    }
    Ok(())
}

/// Remove an offer from storage.
pub fn remove_offer(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Offer(token_id));
}

/// Remove all expired offers. Returns the number of offers removed (#886).
pub fn remove_expired_offers(env: &Env) -> u32 {
    let now = env.ledger().timestamp();
    let mut removed = 0u32;

    // Iterate over a snapshot of offer keys to avoid borrow issues.
    let keys: soroban_sdk::Vec<DataKey> = env.storage().persistent().keys(
        &DataKey::Offer(0),
        soroban_sdk::Limits::none(),
    );

    for i in 0..keys.len() {
        let key = keys.get_unchecked(i);
        if let DataKey::Offer(token_id) = key {
            if let Some(offer) = env
                .storage()
                .persistent()
                .get::<DataKey, Offer>(&key)
            {
                if offer.expires_at > 0 && offer.expires_at <= now {
                    remove_offer(env, token_id);
                    removed += 1;
                }
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
