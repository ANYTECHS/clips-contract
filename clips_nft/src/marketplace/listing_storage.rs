//! Listing storage — persistence layer for marketplace listings.
//!
//! # Storage
//! Key: `DataKey::Listing(token_id)` (persistent storage)

use soroban_sdk::Env;

use crate::types::{DataKey, Error, TokenId};

use super::types::{Listing, ListingStatus};

/// Save a listing. Overwrites any existing listing for the same token.
pub fn save_listing(env: &Env, listing: &Listing) {
    env.storage()
        .persistent()
        .set(&DataKey::Listing(listing.token_id), listing);
}

/// Load a listing by token ID. Returns `Err(TokenNotFound)` if absent.
pub fn get_listing(env: &Env, token_id: TokenId) -> Result<Listing, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Listing(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Check whether a listing exists for the given token.
pub fn has_listing(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Listing(token_id))
}

/// Update the status of an existing listing.
pub fn update_listing_status(
    env: &Env,
    token_id: TokenId,
    status: ListingStatus,
) -> Result<(), Error> {
    let mut listing = get_listing(env, token_id)?;
    listing.status = status;
    save_listing(env, &listing);
    Ok(())
}

/// Remove a listing from storage.
pub fn remove_listing(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Listing(token_id));
}
