//! Marketplace data types — listings, offers, and status enums.

use soroban_sdk::{Address, BytesN, ContractClient, Env};

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
}

/// A binding buy offer for an NFT.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Offer {
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
    /// Unix timestamp (seconds) when the offer was placed.
    pub created_at: u64,
}
