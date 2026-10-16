//! Marketplace module — NFT listing, offer, and purchase logic.
//!
//! # Sub-modules
//! - [`types`] — domain types (Listing, Offer, event structs)
//! - [`listing_storage`] — persistent CRUD for marketplace Listing records
//! - [`listing_validator`] — pre-listing validation
//! - [`purchase_validator`] — pre-purchase validation
//! - [`offer_storage`] — persistent CRUD for marketplace Offer records

pub mod listing_storage;
pub mod listing_validator;
pub mod offer_storage;
pub mod purchase_state_guard;
pub mod purchase_validator;
pub mod types;

use soroban_sdk::{Address, Env};

use crate::types::{Error, ListingId, TokenId};
use crate::ListingRequest;

/// High-level list-NFT entry point used by `ClipsNftContract::list_nft`.
///
/// Validates the listing, generates a listing ID, persists it via the
/// top-level `listing_storage` (using `ListingRequest`), and emits the
/// listing-created event.
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
