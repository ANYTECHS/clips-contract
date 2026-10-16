//! Marketplace storage shapes.
//!
//! `Listing` / `Offer` are the canonical on-chain records. Event payloads
//! (`OfferCreatedEvent`, `OfferAcceptedEvent`) mirror the data indexers need
//! without re-reading storage.

use soroban_sdk::{contracttype, Address};

use crate::types::TokenId;

/// Lifecycle state of a marketplace listing.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
}

/// An active marketplace listing for one token.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    pub token_id: TokenId,
    pub seller: Address,
    pub price: i128,
    pub payment_asset: Address,
    pub expires_at: u64,
    pub status: ListingStatus,
    pub created_at: u64,
    pub buyer: Option<Address>,
    pub sold_at: Option<u64>,
}

/// Lifecycle state of a marketplace offer.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferStatus {
    Active,
    Accepted,
    Cancelled,
    Expired,
}

/// A buyer's open offer for one token. One open offer per token
/// (`DataKey::Offer(token_id)`); creating another fails with
/// `Error::OfferAlreadyExists`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offer {
    pub offer_id: u64,
    pub token_id: TokenId,
    pub buyer: Address,
    pub price: i128,
    pub payment_asset: Address,
    pub expires_at: u64,
    pub status: OfferStatus,
    pub created_at: u64,
}

/// Event payload emitted when an offer is created (`ofr_crt`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferCreatedEvent {
    pub offer_id: u64,
    pub token_id: TokenId,
    pub buyer: Address,
    pub offer_amount: i128,
    pub asset: Address,
    pub expiration: u64,
}

/// Event payload emitted when an offer is accepted (`ofr_acc`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferAcceptedEvent {
    pub offer_id: u64,
    pub token_id: TokenId,
    pub buyer: Address,
    pub seller: Address,
    pub accepted_amount: i128,
    pub timestamp: u64,
}
