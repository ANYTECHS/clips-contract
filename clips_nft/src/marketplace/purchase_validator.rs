//! Purchase validation for NFT marketplace listings.

use soroban_sdk::{token, Address, Env};

use super::listing_storage;
use super::types::{Listing, ListingStatus};
use crate::{
    blacklist, pause_guard, payment_currency, purchase_request::PurchaseRequest,
    token_owner_storage,
    types::{Error, TokenId},
};

/// Validate every precondition for a direct marketplace purchase.
pub fn validate_purchase(
    env: &Env,
    buyer: &Address,
    listing: &Listing,
    payment_asset: &Address,
    amount: i128,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;

    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }

    if listing.expires_at != 0 && listing.expires_at <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, buyer)
        || blacklist::is_blacklisted(env, &listing.seller)
    {
        return Err(Error::Unauthorized);
    }
    if payment_asset != &listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if amount != listing.price {
        return Err(Error::IncorrectPaymentAmount);
    }
    if amount <= 0 {
        return Err(Error::InvalidSalePrice);
    }

    let buyer_balance = token::Client::new(env, payment_asset).balance(buyer);
    if buyer_balance < amount {
        return Err(Error::InsufficientBalance);
    }

    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;
    Ok(())
}

/// Load a listing and validate a direct purchase against it.
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    payment_asset: &Address,
    amount: i128,
) -> Result<Listing, Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    validate_purchase(env, buyer, &listing, payment_asset, amount)?;
    Ok(listing)
}

/// Validate a purchase request whose `max_price` is allowed to cover the listing.
pub fn validate_purchase_request(
    env: &Env,
    req: &PurchaseRequest,
    listing: &Listing,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    if listing.expires_at != 0 && listing.expires_at <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    if req.buyer == listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if req.payment_asset != listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, &req.payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if req.max_price < listing.price {
        return Err(Error::InvalidSalePrice);
    }

    let buyer_balance = token::Client::new(env, &req.payment_asset).balance(&req.buyer);
    if buyer_balance < listing.price {
        return Err(Error::InsufficientBalance);
    }
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;
    Ok(())
}
