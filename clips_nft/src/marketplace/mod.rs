//! Marketplace module — listings, offers, and purchase validation.
//!
//! Centralizes the marketplace domain behind one module tree so the contract
//! entry points (`list_nft`, `buy_listing`, `make_offer`, `accept_offer`,
//! `cancel_offer`) and off-chain indexers share a single source of truth for
//! storage shapes and validation rules.
//!
//! Submodules mirror the existing flat-file patterns (`listing_storage.rs`,
//! `purchase_validator.rs`) so behavior stays consistent:
//!
//! * [`types`] — `Listing` / `Offer` storage shapes and event payloads.
//! * [`listing_storage`] — active-listing persistence keyed by token.
//! * [`offer_storage`] — open-offer persistence keyed by token.
//! * [`listing_validator`] — pre-conditions for creating a listing.
//! * [`purchase_validator`] — pre-conditions for buying a listing.

pub mod listing_storage;
pub mod listing_validator;
pub mod offer_storage;
pub mod purchase_validator;
pub mod types;

pub use types::{Listing, ListingStatus, Offer, OfferStatus};
