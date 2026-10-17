//! Marketplace domain types.
//!
//! Defines every data structure used across the marketplace sub-modules:
//! - [`Listing`] / [`ListingStatus`] — seller-created NFT sale listings.
//! - [`Offer`] / [`OfferStatus`] — buyer-originated bid offers.
//! - Event structs emitted during each lifecycle transition.

use soroban_sdk::{contracttype, Address};

use crate::types::{ListingId, TokenId};

/// Status of a marketplace listing.
///
/// Represents the full lifecycle of a marketplace listing:
/// - `Active`    — Listing is live and accepting purchases.
/// - `Sold`      — The NFT has been sold; listing is closed.
/// - `Cancelled` — The seller (or an authorized party) cancelled the listing.
/// - `Expired`   — The listing's expiration timestamp has passed without a sale.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    /// Listing is active and can be purchased.
    Active,
    /// NFT has been sold via this listing.
    Sold,
    /// Listing was cancelled by the seller.
    Cancelled,
    /// Listing has expired (expiration timestamp passed without a purchase).
    Expired,
}

/// A marketplace listing created by a seller.
///
/// # Fields
/// - `token_id`       — On-chain token identifier being listed.
/// - `seller`         — Address of the seller.
/// - `price`          — Asking price in the payment asset's smallest unit.
/// - `payment_asset`  — Accepted payment token contract address.
/// - `expires_at`     — Unix timestamp; `0` means the listing never expires.
/// - `status`         — Current lifecycle state.
/// - `created_at`     — Ledger timestamp at creation.
/// - `buyer`          — Populated on sale completion; `None` until sold.
/// - `sold_at`        — Ledger timestamp at sale completion; `None` until sold.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    /// On-chain token identifier being offered for sale.
    pub token_id: TokenId,
    /// Address of the seller.
    pub seller: Address,
    /// Asking price in the payment asset's smallest unit.
    pub price: i128,
    /// Payment token accepted for this listing.
    pub payment_asset: Address,
    /// Unix expiration timestamp; `0` means never expires.
    pub expires_at: u64,
    /// Current lifecycle state of the listing.
    pub status: ListingStatus,
    /// Ledger timestamp when the listing was created.
    pub created_at: u64,
    /// Buyer address, populated once the NFT is sold.
    pub buyer: Option<Address>,
    /// Ledger timestamp when the NFT was sold, if applicable.
    pub sold_at: Option<u64>,
}

// ── Offer ─────────────────────────────────────────────────────────────────────

/// Current status of a buyer offer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OfferStatus {
    /// Offer is active and can be accepted by the seller.
    Active,
    /// Offer was accepted by the seller.
    Accepted,
    /// Offer was cancelled by the buyer (or an authorized party).
    Cancelled,
    /// Offer expired without acceptance.
    Expired,
}

/// A buyer-originated offer on an NFT.
///
/// # Fields
/// - `offer_id`      — Unique offer identifier.
/// - `token_id`      — On-chain token identifier the offer targets.
/// - `buyer`         — Address of the buyer who placed the offer.
/// - `price`         — Offered price in the payment asset's smallest unit.
/// - `payment_asset` — Payment token contract address.
/// - `expires_at`    — Unix timestamp; `0` means the offer never expires.
/// - `status`        — Current lifecycle state.
/// - `created_at`    — Ledger timestamp at offer creation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offer {
    /// Unique offer identifier.
    pub offer_id: u64,
    /// On-chain token identifier the offer targets.
    pub token_id: TokenId,
    /// Address that placed the offer.
    pub buyer: Address,
    /// Offered price in the payment asset's smallest unit.
    pub price: i128,
    /// Payment token contract address.
    pub payment_asset: Address,
    /// Unix expiration timestamp; `0` means never expires.
    pub expires_at: u64,
    /// Current lifecycle state.
    pub status: OfferStatus,
    /// Ledger timestamp at offer creation.
    pub created_at: u64,
}

// ── Marketplace event types ───────────────────────────────────────────────────

/// Event emitted when an NFT is listed for sale.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftListedEvent {
    /// Unique listing identifier.
    pub listing_id: ListingId,
    /// On-chain token identifier.
    pub token_id: TokenId,
    /// Seller address.
    pub seller: Address,
    /// Asking price.
    pub price: i128,
    /// Payment asset contract address.
    pub asset: Address,
    /// Ledger timestamp.
    pub timestamp: u64,
}

/// Event emitted when an active listing is cancelled.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCancelledEvent {
    /// Unique listing identifier.
    pub listing_id: ListingId,
    /// On-chain token identifier.
    pub token_id: TokenId,
    /// Address of the seller.
    pub seller: Address,
    /// Ledger timestamp.
    pub timestamp: u64,
}

/// Event emitted when an NFT is successfully sold.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftSoldEvent {
    /// Unique listing identifier.
    pub listing_id: ListingId,
    /// On-chain token identifier.
    pub token_id: TokenId,
    /// Previous owner (seller).
    pub seller: Address,
    /// New owner (buyer).
    pub buyer: Address,
    /// Final sale price.
    pub sale_amount: i128,
    /// Payment asset used.
    pub payment_asset: Address,
    /// Ledger timestamp.
    pub timestamp: u64,
}

/// Event emitted when a buyer places an offer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferCreatedEvent {
    /// Unique offer identifier.
    pub offer_id: u64,
    /// On-chain token identifier.
    pub token_id: TokenId,
    /// Buyer who placed the offer.
    pub buyer: Address,
    /// Offered price.
    pub offer_amount: i128,
    /// Payment asset.
    pub asset: Address,
    /// Offer expiration timestamp.
    pub expiration: u64,
}

/// Event emitted when a seller accepts a buyer's offer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferAcceptedEvent {
    /// Unique offer identifier.
    pub offer_id: u64,
    /// On-chain token identifier.
    pub token_id: TokenId,
    /// Buyer whose offer was accepted.
    pub buyer: Address,
    /// Seller who accepted the offer.
    pub seller: Address,
    /// Accepted price.
    pub accepted_amount: i128,
    /// Ledger timestamp.
    pub timestamp: u64,
}
