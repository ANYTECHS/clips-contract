//! Purchase state guard — verifies an NFT listing is in a purchasable state (issue #1027).

use soroban_sdk::Env;

use crate::listing_storage;
use crate::types::{Error, ListingStatus, TokenId};
use crate::ListingRequest;

/// Return `Ok(listing)` if the listing for `token_id` exists.
pub fn check_listing_exists(env: &Env, token_id: TokenId) -> Result<ListingRequest, Error> {
    listing_storage::get_listing(env, token_id)
}

/// Return `Ok(())` if the listing is in `Active` state.
pub fn check_listing_active(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    match listing.status {
        ListingStatus::Active => Ok(()),
        ListingStatus::Sold => Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => Err(Error::ListingNotActive),
        ListingStatus::Expired => Err(Error::ListingExpired),
    }
}

/// Return `Ok(())` if the listing has not expired.
pub fn check_listing_not_expired(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    if listing.expiration != 0 && listing.expiration <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    Ok(())
}

/// Return `Ok(())` if the listing has not been sold.
pub fn check_listing_not_sold(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    match listing.status {
        ListingStatus::Sold => Err(Error::ListingAlreadySold),
        _ => Ok(()),
    }
}

/// Return the listing if all purchase preconditions pass.
pub fn get_purchasable_listing(env: &Env, token_id: TokenId) -> Result<ListingRequest, Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }
    if listing.expiration != 0 && listing.expiration <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    Ok(listing)
}

/// Require listing is purchasable or return an error.
pub fn require_purchasable(env: &Env, token_id: TokenId) -> Result<(), Error> {
    get_purchasable_listing(env, token_id).map(|_| ())
}

/// Alias for [`require_purchasable`].
pub fn require_purchasable_listing(env: &Env, token_id: TokenId) -> Result<(), Error> {
    require_purchasable(env, token_id)
}
