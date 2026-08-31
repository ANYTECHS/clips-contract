//! Marketplace module — NFT listing, purchasing, and offer functionality.
//!
//! This module provides the core primitives for a peer-to-peer NFT marketplace:
//!
//! - **Listings**: sellers offer an NFT at a fixed price in a supported payment
//!   asset. Listings have an optional expiration timestamp.
//! - **Offers**: buyers place binding offers on listed (or unlisted) NFTs.
//! - **Settlement**: when a buyer purchases or an offer is accepted, the contract
//!   transfers ownership and distributes royalties according to the token's
//!   configured royalty split.
//!
//! # Architecture
//!
//! ```text
//! marketplace/
//! ├── mod.rs              ← this file: re-exports and module docs
//! ├── listing_storage.rs  ← persistence for active listings
//! ├── listing_validator.rs← reusable pre-condition checks for listings
//! ├── offer_storage.rs    ← persistence for open offers
//! └── types.rs            ← Listing, Offer, and related data types
//! ```
//!
//! All state-changing entry points must call [`crate::pause_guard::require_not_paused`]
//! and [`crate::royalty_pause_guard::require_royalty_not_paused`] before
//! executing. Listing-specific validation is centralised in
//! [`listing_validator`].

pub mod listing;
pub mod listing_storage;
pub mod listing_validator;
pub mod offer_storage;
pub mod purchase_validator;
pub mod types;

pub use listing::list_nft;
pub use purchase_validator::{
    validate_purchase, validate_purchase_for_token, validate_purchase_request,
};
pub use types::{
    Listing, ListingCancelledEvent, ListingStatus, NftListedEvent, NftSoldEvent, Offer,
    OfferAcceptedEvent, OfferCreatedEvent, OfferStatus, PurchaseRequest,
};

