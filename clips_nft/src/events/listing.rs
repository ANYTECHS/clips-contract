//! Marketplace listing and sale events.
//!
//! Defines every event emitted by the listing lifecycle and exposes a small
//! `emit_*` helper per event. All emission logic is centralized here so the
//! rest of the contract never publishes a raw topic string.

use soroban_sdk::{contracttype, symbol_short, Address, Env};

use crate::types::{ListingId, TokenId};

/// Emitted when a seller creates a new marketplace listing (#862).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCreatedEvent {
    /// Token ID that was listed.
    pub token_id: TokenId,
    /// Address of the seller who created the listing.
    pub seller: Address,
    /// Asking price in stroops.
    pub price: i128,
    /// Accepted payment asset contract address.
    pub payment_asset: Address,
    /// Unix expiration timestamp (`0` = never expires).
    pub expires_at: u64,
    /// Unix timestamp of creation.
    pub timestamp: u64,
}

/// Emitted when a seller updates an active listing's price or expiration (#871).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingUpdatedEvent {
    /// Unique identifier of the updated listing.
    pub listing_id: ListingId,
    /// Token ID of the updated listing.
    pub token_id: TokenId,
    /// Seller who performed the update.
    pub seller: Address,
    /// Previous asking price in stroops.
    pub old_price: i128,
    /// New asking price in stroops.
    pub new_price: i128,
    /// Previous expiration timestamp.
    pub old_expires_at: u64,
    /// New expiration timestamp.
    pub new_expires_at: u64,
    /// Unix timestamp of the update.
    pub timestamp: u64,
}

/// Emitted when a listing is cancelled by the seller or an authorized operator (#924).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCancelledEvent {
    /// Token ID of the cancelled listing.
    pub token_id: TokenId,
    /// Seller who originally created the listing.
    pub seller: Address,
    /// Address that performed the cancellation (may differ from `seller`).
    pub cancelled_by: Address,
    /// Unix timestamp of the cancellation.
    pub timestamp: u64,
}

/// Emitted when an NFT is sold through a marketplace purchase (#884).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftSoldEvent {
    /// Token ID that was sold.
    pub token_id: TokenId,
    /// Seller who received the proceeds.
    pub seller: Address,
    /// Buyer who purchased the token.
    pub buyer: Address,
    /// Sale amount in stroops.
    pub sale_amount: i128,
    /// Payment asset contract address.
    pub payment_asset: Address,
    /// Unix timestamp of the sale.
    pub timestamp: u64,
}

/// Emit [`ListingCreatedEvent`].
pub fn emit_listing_created(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("lst_crt"),),
        ListingCreatedEvent {
            token_id,
            seller: seller.clone(),
            price,
            payment_asset: payment_asset.clone(),
            expires_at,
            timestamp,
        },
    );
}

/// Emit [`ListingUpdatedEvent`].
pub fn emit_listing_updated(
    env: &Env,
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    old_price: i128,
    new_price: i128,
    old_expires_at: u64,
    new_expires_at: u64,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("lst_upd"),),
        ListingUpdatedEvent {
            listing_id,
            token_id,
            seller: seller.clone(),
            old_price,
            new_price,
            old_expires_at,
            new_expires_at,
            timestamp,
        },
    );
}

/// Emit [`ListingCancelledEvent`].
pub fn emit_listing_cancelled(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    cancelled_by: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("lst_can"),),
        ListingCancelledEvent {
            token_id,
            seller: seller.clone(),
            cancelled_by: cancelled_by.clone(),
            timestamp,
        },
    );
}

/// Emit [`NftSoldEvent`].
pub fn emit_nft_sold(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    sale_amount: i128,
    payment_asset: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_sold"),),
        NftSoldEvent {
            token_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            sale_amount,
            payment_asset: payment_asset.clone(),
            timestamp,
        },
    );
}
