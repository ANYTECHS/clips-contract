//! Active-listing persistence, keyed by token.
//!
//! Mirrors the flat `listing_storage.rs` pattern but stores the canonical
//! [`Listing`](super::types::Listing) shape under `DataKey::ActiveListing`.
//! `save_listing` overwrites unconditionally: callers run
//! [`validate_listing`](super::listing_validator::validate_listing) first,
//! which rejects duplicates with `Error::DuplicateRecord`.

use soroban_sdk::Env;

use super::types::{Listing, ListingStatus};
use crate::types::{DataKey, Error, TokenId};

/// Persist (create or overwrite) the active listing for a token.
pub fn save_listing(env: &Env, listing: &Listing) {
    env.storage()
        .persistent()
        .set(&DataKey::ActiveListing(listing.token_id), listing);
}

/// Load the active listing for a token, if any.
pub fn get_listing(env: &Env, token_id: TokenId) -> Option<Listing> {
    env.storage()
        .persistent()
        .get(&DataKey::ActiveListing(token_id))
}

/// Remove the active listing (cancelled or sold).
pub fn remove_listing(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let key = DataKey::ActiveListing(token_id);
    if !env.storage().persistent().has(&key) {
        return Err(Error::ListingNotFound);
    }
    env.storage().persistent().remove(&key);
    Ok(())
}

/// True when an *active* listing exists for the token. Sold or cancelled
/// records do not block relisting.
pub fn has_active_listing(env: &Env, token_id: TokenId) -> bool {
    matches!(
        get_listing(env, token_id),
        Some(Listing {
            status: ListingStatus::Active,
            ..
        })
    )
}
