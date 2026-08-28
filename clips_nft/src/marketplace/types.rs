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
