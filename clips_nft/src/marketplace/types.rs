//! Marketplace storage shapes.

use soroban_sdk::{contracttype, Address};
use crate::types::{ListingId, TokenId};

pub const MAX_MARKETPLACE_PRICE: i128 = i128::MAX / 2;

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    pub token_id: TokenId,
    pub seller: Address,
    pub payment_asset: Address,
    pub price: i128,
    pub created_at: u64,
    pub active: bool,
    pub status: ListingStatus,
    pub expires_at: u64,
    pub buyer: Option<Address>,
    pub sold_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferStatus {
    Active,
    Accepted,
    Cancelled,
    Expired,
}

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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftListedEvent {
    pub listing_id: ListingId,
    pub token_id: TokenId,
    pub seller: Address,
    pub price: i128,
    pub asset: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCancelledEvent {
    pub listing_id: ListingId,
    pub token_id: TokenId,
    pub seller: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftSoldEvent {
    pub listing_id: ListingId,
    pub token_id: TokenId,
    pub seller: Address,
    pub buyer: Address,
    pub sale_amount: i128,
    pub payment_asset: Address,
    pub timestamp: u64,
}

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
