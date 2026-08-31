//! Marketplace offer events.
//!
//! Defines every event emitted by the offer lifecycle (a buyer's bid) and
//! exposes one `emit_*` helper per event. Emission logic is centralized here so
//! callers never publish raw topic strings.

use soroban_sdk::{contracttype, symbol_short, Address, Env};

use crate::types::TokenId;

/// Emitted when a buyer places an offer on a token (#884).
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

/// Emitted when a seller accepts a buyer's offer (#884).
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

/// Emitted when an offer is cancelled by the buyer or an authorized operator (#884).
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
        (symbol_short!("ofr_made"),),
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
        (symbol_short!("ofr_acc"),),
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
        (symbol_short!("ofr_can"),),
        OfferCancelledEvent {
            token_id,
            buyer: buyer.clone(),
            cancelled_by: cancelled_by.clone(),
            timestamp,
        },
    );
}
