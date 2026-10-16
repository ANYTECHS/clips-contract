//! Marketplace module — NFT listing, purchasing, and offer functionality.
//!
//! This module is the single entry point for all on-chain marketplace logic
//! in the ClipCash NFT contract. It organises listing creation, offer
//! management, and purchase validation into focused sub-modules while
//! re-exporting the most-used types at the module root so callers do not need
//! to reach into sub-modules directly.
//!
//! # Architecture
//!
//! ```text
//! marketplace/
//! ├── mod.rs              ← you are here — high-level entry points and re-exports
//! ├── types.rs            ← domain types: Listing, Offer, event structs
//! ├── listing_storage.rs  ← persistent CRUD for Listing records
//! ├── listing_validator.rs← pre-listing validation pipeline
//! ├── offer_storage.rs    ← persistent CRUD for Offer records
//! ├── purchase_state_guard.rs ← guards that verify a listing is purchasable
//! └── purchase_validator.rs   ← full purchase precondition validation
//! ```
//!
//! # Listing logic
//!
//! A seller creates a listing by calling [`list_nft`]. The function runs the
//! full validation pipeline ([`listing_validator`]), generates a listing ID,
//! persists the record ([`listing_storage`]), and emits a listing-created
//! event so off-chain indexers can track active listings.
//!
//! # Offer logic
//!
//! Buyer offers are stored as [`Offer`] records in [`offer_storage`]. The
//! offer lifecycle (creation, acceptance, cancellation, expiry) is tracked
//! through [`OfferStatus`] and the associated event structs defined in
//! [`types`].
//!
//! # Purchase logic
//!
//! [`purchase_validator`] checks all preconditions (pause state, listing
//! activity, payment asset, price match) before a purchase is executed.
//! [`purchase_state_guard`] provides lightweight individual guards that can
//! be composed when only a subset of checks is needed.

// ── Sub-modules ───────────────────────────────────────────────────────────────

/// Domain types: [`Listing`], [`ListingStatus`], [`Offer`], [`OfferStatus`],
/// and marketplace event structs.
pub mod types;

/// Persistent CRUD operations for [`Listing`] records, keyed by token ID.
pub mod listing_storage;

/// Validation pipeline for new marketplace listings — checks pause state,
/// ownership, price bounds, supported asset, and duplicate detection.
pub mod listing_validator;

/// Persistent CRUD operations for [`Offer`] records, keyed by token ID.
pub mod offer_storage;

/// Individual purchase-state guards that verify a listing is purchasable.
/// Each guard tests one condition and can be composed into larger pipelines.
pub mod purchase_state_guard;

/// Full purchase precondition validator — combines pause, buyer-seller
/// identity, listing status, expiry, asset match, and price match checks.
pub mod purchase_validator;

// ── Re-exports ────────────────────────────────────────────────────────────────

// Listing types
pub use types::{Listing, ListingStatus};

// Offer types
pub use types::{Offer, OfferStatus};

// Marketplace event structs
pub use types::{
    ListingCancelledEvent, NftListedEvent, NftSoldEvent, OfferAcceptedEvent, OfferCreatedEvent,
};

// Listing storage helpers
pub use listing_storage::{
    get_listing, has_listing, remove_listing, save_listing, try_get_listing, update_listing,
    update_listing_status,
};

// Offer storage helpers
pub use offer_storage::{get_offer, has_offer, remove_offer, save_offer, update_offer};

// Purchase validation
pub use purchase_validator::{validate_purchase, validate_purchase_for_token};

// Purchase state guards
pub use purchase_state_guard::{
    check_listing_active, check_listing_exists, check_listing_not_expired,
    check_listing_not_sold, get_purchasable_listing, require_purchasable,
};

// ── High-level entry points ───────────────────────────────────────────────────

use soroban_sdk::{Address, Env};

use crate::types::{Error, ListingId, TokenId};
use crate::ListingRequest;

/// List an NFT for sale in the marketplace.
///
/// This is the single entry point for creating a marketplace listing. It
/// runs the full validation pipeline, generates a unique listing ID, persists
/// the record, and emits a listing-created event for off-chain indexers.
///
/// # Arguments
/// * `env`     — Contract execution environment.
/// * `request` — Listing parameters supplied by the seller.
///
/// # Returns
/// The unique [`ListingId`] assigned to the new listing.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ContractPaused`   | Contract is globally paused. |
/// | `TokenNotFound`    | NFT does not exist. |
/// | `Unauthorized`     | Caller is not the token owner. |
/// | `InvalidSalePrice` | Price is zero or negative. |
/// | `PriceOverflow`    | Price exceeds the maximum allowed value. |
/// | `UnsupportedAsset` | Payment asset is not a supported currency. |
/// | `DuplicateListing` | An active listing already exists for this token. |
pub fn list_nft(env: &Env, request: &ListingRequest) -> Result<ListingId, Error> {
    listing_validator::validate_listing(
        env,
        &request.seller,
        request.token_id,
        request.price,
        &request.payment_asset,
        request.expiration,
    )?;

    let mut req = request.clone();
    let listing_id = crate::listing_storage::create_listing(env, &mut req)?;

    crate::events::listing::emit_listing_created(
        env,
        request.token_id,
        &request.seller,
        request.price,
        &request.payment_asset,
        request.expiration,
        env.ledger().timestamp(),
    );

    Ok(listing_id)
}
