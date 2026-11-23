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
pub mod offer_acceptance;
pub mod offer_storage;
pub mod offer_validator;
pub mod purchase_validator;
pub mod purchase_state_guard;
pub mod types;

pub use types::{Listing, ListingStatus, Offer, OfferStatus, MAX_MARKETPLACE_PRICE};

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

/// Update an active marketplace listing's price and/or expiration (Issue #1325).
///
/// Validates seller authority, verifies active listing status, validates new values,
/// updates the listing in storage, and emits the ListingUpdated event.
pub fn update_listing_endpoint(
    env: &Env,
    seller: &Address,
    token_id: TokenId,
    new_price: i128,
    new_expiration: u64,
) -> Result<(), Error> {
    crate::listing_storage::update_listing(env, seller, token_id, new_price, new_expiration)
}

