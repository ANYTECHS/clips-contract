//! Marketplace data types — listings, offers, and status enums.

use soroban_sdk::{contracttype, Address, Env};

use crate::types::TokenId;

/// Status of a marketplace listing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ListingStatus {
    /// Listing is active and accepting purchases.
    Active,
    /// Listing has been sold.
    Sold,
    /// Listing was cancelled by the seller or expired.
    Cancelled,
}

/// A fixed-price NFT listing.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Listing {
    /// Token being listed for sale.
    pub token_id: TokenId,
    /// Seller's Stellar address.
    pub seller: Address,
    /// Asking price in stroops (smallest unit of the payment asset).
    pub price: i128,
    /// Soroban address of the accepted payment asset contract.
    pub payment_asset: Address,
    /// Unix timestamp (seconds) after which the listing expires. `0` means no
    /// expiration.
    pub expires_at: u64,
    /// Current listing status.
    pub status: ListingStatus,
    /// Unix timestamp (seconds) when the listing was created.
    pub created_at: u64,
    /// Buyer's Stellar address, set after a successful purchase (#883).
    pub buyer: Option<Address>,
    /// Unix timestamp (seconds) when the listing was sold (#883).
    pub sold_at: Option<u64>,
}

/// Status of a marketplace offer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum OfferStatus {
    /// Offer is active and awaiting acceptance.
    Active,
    /// Offer was accepted and the NFT was transferred.
    Accepted,
    /// Offer was cancelled by the buyer or expired.
    Cancelled,
}

/// A binding buy offer for an NFT (#885).
#[derive(Clone, Debug)]
#[contracttype]
pub struct Offer {
    /// Unique offer identifier.
    pub offer_id: u64,
    /// Token the offer targets. May or may not be actively listed.
    pub token_id: TokenId,
    /// Buyer's Stellar address.
    pub buyer: Address,
    /// Offered price in stroops.
    pub price: i128,
    /// Soroban address of the payment asset contract.
    pub payment_asset: Address,
    /// Unix timestamp (seconds) after which the offer expires. `0` means no
    /// expiration.
    pub expires_at: u64,
    /// Current offer status.
    pub status: OfferStatus,
    /// Unix timestamp (seconds) when the offer was placed.
    pub created_at: u64,
}

pub use crate::events::listing::{ListingCancelledEvent, NftSoldEvent};
/// Emitted when an NFT is sold via marketplace purchase (#884 / issue #925).
///
/// # Fields (issue #925 acceptance criteria)
/// * `listing_id`    — Identifier of the marketplace listing that was sold.
/// * `token_id`      — Token ID of the sold NFT.
/// * `buyer`         — Buyer's address.
/// * `seller`        — Seller's address.
/// * `sale_amount`   — Sale amount in stroops.
/// * `payment_asset` — Payment asset contract address.
/// * `timestamp`     — Unix timestamp of the sale.
#[derive(Clone, Debug)]
#[contracttype]
pub struct NftSoldEvent {
    /// Identifier of the marketplace listing that was sold.
    pub listing_id: ListingId,
    /// Token ID of the sold NFT.
    pub token_id: TokenId,
    /// Seller's address.
    pub seller: Address,
    /// Buyer's address.
    pub buyer: Address,
    /// Sale amount in stroops.
    pub sale_amount: i128,
    /// Payment asset contract address.
    pub payment_asset: Address,
    /// Unix timestamp of the sale.
    pub timestamp: u64,
}

/// Emitted whenever a buyer creates an offer for an NFT (issue #926).
///
/// # Fields (issue #926 acceptance criteria)
/// * `offer_id`      — Unique identifier of the created offer.
/// * `token_id`      — Token the offer targets.
/// * `buyer`         — Buyer's (offerer's) address.
/// * `offer_amount`  — Offered price in stroops.
/// * `asset`         — Payment asset contract address.
/// * `expiration`    — Unix timestamp after which the offer expires.
#[derive(Clone, Debug)]
#[contracttype]
pub struct OfferCreatedEvent {
    /// Unique identifier of the created offer.
    pub offer_id: u64,
    /// Token the offer targets.
    pub token_id: TokenId,
    /// Buyer's (offerer's) address.
    pub buyer: Address,
    /// Offered price in stroops.
    pub offer_amount: i128,
    /// Payment asset contract address.
    pub asset: Address,
    /// Unix timestamp after which the offer expires.
    pub expiration: u64,
}

/// Emitted after an NFT owner accepts a marketplace offer (issue #927).
///
/// # Fields (issue #927 acceptance criteria)
/// * `offer_id`         — Unique identifier of the accepted offer.
/// * `token_id`         — Token the offer targets.
/// * `buyer`            — Buyer's (offerer's) address.
/// * `seller`           — Seller's (acceptor's) address.
/// * `accepted_amount`  — Amount the offer was accepted for, in stroops.
/// * `timestamp`        — Unix timestamp of acceptance.
#[derive(Clone, Debug)]
#[contracttype]
pub struct OfferAcceptedEvent {
    /// Unique identifier of the accepted offer.
    pub offer_id: u64,
    /// Token the offer targets.
    pub token_id: TokenId,
    /// Buyer's (offerer's) address.
    pub buyer: Address,
    /// Seller's (acceptor's) address.
    pub seller: Address,
    /// Amount the offer was accepted for, in stroops.
    pub accepted_amount: i128,
    /// Unix timestamp of acceptance.
    pub timestamp: u64,
}

/// Emitted when a marketplace NFT listing is successfully created (issue #873).
///
/// # Fields (issue #873 acceptance criteria)
/// * `listing_id`    — Unique identifier of the marketplace listing.
/// * `token_id`      — On-chain token ID of the listed NFT.
/// * `seller`        — Address of the seller.
/// * `price`         — Asking price in stroops.
/// * `payment_asset` — Address of the accepted payment asset contract.
/// * `timestamp`     — Ledger timestamp in seconds since Unix epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct NftListedEvent {
    /// Unique identifier of the marketplace listing.
    pub listing_id: ListingId,
    /// On-chain token ID of the listed NFT.
    pub token_id: TokenId,
    /// Address of the seller.
    pub seller: Address,
    /// Asking price in stroops.
    pub price: i128,
    /// Address of the accepted payment asset contract.
    pub payment_asset: Address,
    /// Ledger timestamp in seconds since Unix epoch.
    pub timestamp: u64,
}

/// Emitted when an active NFT listing is cancelled (issue #874).
///
/// # Fields (issue #874 acceptance criteria)
/// * `listing_id` — Unique identifier of the cancelled listing.
/// * `token_id`   — Token ID of the cancelled listing.
/// * `seller`     — Seller's address.
/// * `timestamp`  — Unix timestamp of the cancellation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ListingCancelledEvent {
    /// Unique identifier of the cancelled listing.
    pub listing_id: ListingId,
    /// Token ID of the cancelled listing.
    pub token_id: TokenId,
    /// Seller's address.
    pub seller: Address,
    /// Unix timestamp of the cancellation.
    pub timestamp: u64,
}

pub use crate::purchase_request::PurchaseRequest;

