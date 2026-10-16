//! Purchase pre-conditions for buying a listed NFT.
//!
//! Single place where "may this buyer complete this purchase" is decided.
//! Mirrors the check order asserted by the marketplace validator tests:
//! pause → active status → expiry → self-purchase → blacklist → ownership →
//! asset → price.

use soroban_sdk::{Address, Env};

use super::{
    listing_storage::get_listing,
    listing_validator::is_expired,
    types::{Listing, ListingStatus},
};
use crate::{
    blacklist, pause_guard, payment_currency,
    purchase_request::PurchaseRequest,
    token_owner_storage,
    types::{Error, TokenId},
};

/// Validate a direct purchase at an explicit price.
pub fn validate_purchase(
    env: &Env,
    buyer: &Address,
    listing: &Listing,
    payment_asset: &Address,
    price: i128,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    if is_expired(env.ledger().timestamp(), listing.expires_at) {
        return Err(Error::OfferExpired);
    }
    if *buyer == listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, buyer) || blacklist::is_blacklisted(env, &listing.seller) {
        return Err(Error::Unauthorized);
    }
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;
    if *payment_asset != listing.payment_asset
        || !payment_currency::is_supported(env, payment_asset)
    {
        return Err(Error::UnsupportedAsset);
    }
    if price < listing.price {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}

/// Load the active listing for a token and validate the purchase.
/// Returns the listing so callers can settle against its terms.
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    payment_asset: &Address,
    price: i128,
) -> Result<Listing, Error> {
    let listing = get_listing(env, token_id).ok_or(Error::ListingNotFound)?;
    validate_purchase(env, buyer, &listing, payment_asset, price)?;
    Ok(listing)
}

/// Validate a [`PurchaseRequest`] (buyer caps spend via `max_price`).
pub fn validate_purchase_request(
    env: &Env,
    req: &PurchaseRequest,
    listing: &Listing,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    if is_expired(env.ledger().timestamp(), listing.expires_at) {
        return Err(Error::OfferExpired);
    }
    if req.payment_asset != listing.payment_asset
        || !payment_currency::is_supported(env, &req.payment_asset)
    {
        return Err(Error::UnsupportedAsset);
    }
    if req.max_price < listing.price {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}
