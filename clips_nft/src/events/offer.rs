//! Marketplace offer events.
//!
//! Defines every event emitted by the offer lifecycle (a buyer's bid) and
//! exposes one `emit_*` helper per event. Emission logic is centralized here so
//! callers never publish raw topic strings.

use soroban_sdk::{contracttype, Address, Env};

use crate::event_topics::{TOPIC_OFFER_ACCEPT, TOPIC_OFFER_CANCELLED, TOPIC_OFFER_MADE};
use crate::types::TokenId;

/// Emitted when a buyer places an offer on a token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferMadeEvent {
    /// Token ID the offer targets.
    pub token_id: TokenId,
    /// Buyer who placed the offer.
    pub buyer: Address,
    /// Offered price in stroops.
    pub price: i128,
    /// Payment asset contract address.
    pub payment_asset: Address,
    /// Unix expiration timestamp (`0` = never expires).
    pub expires_at: u64,
    /// Unix timestamp of placement.
    pub timestamp: u64,
}

/// Emitted when a seller accepts a buyer's offer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferAcceptedEvent {
    /// Token ID the offer targeted.
    pub token_id: TokenId,
    /// Seller who accepted the offer.
    pub seller: Address,
    /// Buyer whose offer was accepted.
    pub buyer: Address,
    /// Accepted price in stroops.
    pub price: i128,
    /// Payment asset contract address.
    pub payment_asset: Address,
    /// Unix timestamp of acceptance.
    pub timestamp: u64,
}

/// Emitted when an offer is cancelled by the buyer or an authorized operator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferCancelledEvent {
    /// Token ID the offer targeted.
    pub token_id: TokenId,
    /// Buyer who originally placed the offer.
    pub buyer: Address,
    /// Address that performed the cancellation (may differ from `buyer`).
    pub cancelled_by: Address,
    /// Unix timestamp of the cancellation.
    pub timestamp: u64,
}

/// Emit [`OfferMadeEvent`].
pub fn emit_offer_made(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
    timestamp: u64,
) {
    env.events().publish(
        (TOPIC_OFFER_MADE,),
        OfferMadeEvent {
            token_id,
            buyer: buyer.clone(),
            price,
            payment_asset: payment_asset.clone(),
            expires_at,
            timestamp,
        },
    );
}

/// Emit [`OfferAcceptedEvent`].
pub fn emit_offer_accepted(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    price: i128,
    payment_asset: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (TOPIC_OFFER_ACCEPT,),
        OfferAcceptedEvent {
            token_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            price,
            payment_asset: payment_asset.clone(),
            timestamp,
        },
    );
}

/// Emit [`OfferCancelledEvent`].
pub fn emit_offer_cancelled(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    cancelled_by: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (TOPIC_OFFER_CANCELLED,),
        OfferCancelledEvent {
            token_id,
            buyer: buyer.clone(),
            cancelled_by: cancelled_by.clone(),
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn emit_offer_made_publishes_one_event() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let asset = Address::generate(env);
            emit_offer_made(env, 1, &buyer, 1_000, &asset, 0, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_offer_accepted_publishes_one_event() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = Address::generate(env);
            emit_offer_accepted(env, 1, &seller, &buyer, 1_000, &asset, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_offer_cancelled_publishes_one_event() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let canceller = Address::generate(env);
            emit_offer_cancelled(env, 1, &buyer, &canceller, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn multiple_events_emit_independently() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let asset = Address::generate(env);
            emit_offer_made(env, 1, &buyer, 1_000, &asset, 0, 100);
            emit_offer_accepted(env, 1, &Address::generate(env), &buyer, 1_000, &asset, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }
}
