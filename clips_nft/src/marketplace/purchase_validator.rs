//! Marketplace purchase validator.

use soroban_sdk::{Address, Env};

use crate::marketplace::types::{Listing, ListingStatus};
use crate::pause_state;
use crate::payment_currency;
use crate::purchase_request::PurchaseRequest;
use crate::types::{Error, TokenId};

/// Validate all preconditions for purchasing an NFT listing.
pub fn validate_purchase(
    env: &Env,
    buyer: &Address,
    listing: &Listing,
    payment_asset: &Address,
    amount: i128,
) -> Result<(), Error> {
    if pause_state::get_pause_state(env) {
        return Err(Error::ContractPaused);
    }
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }
    let now = env.ledger().timestamp();
    if listing.expires_at != 0 && listing.expires_at <= now {
        return Err(Error::ListingExpired);
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
    Ok(())
}

/// Validate a purchase request against the stored listing for `token_id`.
pub fn validate_purchase_for_token(
    env: &Env,
    buyer: &Address,
    token_id: TokenId,
    payment_asset: &Address,
    amount: i128,
) -> Result<Listing, Error> {
    let listing = crate::marketplace::listing_storage::get_listing(env, token_id)?;
    validate_purchase(env, buyer, &listing, payment_asset, amount)?;
    Ok(listing)
}

/// Validate using a [`PurchaseRequest`] struct.
pub fn validate_purchase_request(
    env: &Env,
    buyer: &Address,
    request: &PurchaseRequest,
) -> Result<Listing, Error> {
    let listing = crate::marketplace::listing_storage::get_listing(env, request.listing_id as TokenId)?;
    validate_purchase(env, buyer, &listing, &request.payment_asset, request.max_price)?;
    Ok(listing)
}
