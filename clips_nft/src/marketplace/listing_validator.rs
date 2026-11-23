//! Marketplace listing validator.
//!
//! Checks all preconditions before a new listing is accepted.
//!
//! # Validation order
//! 1. Contract is not paused.
//! 2. Caller is not blacklisted.
//! 3. Caller is the token owner (else `UnauthorizedOwnerError`).
//! 4. Price is positive and within `MAX_LISTING_PRICE`.
//! 5. Payment asset is a supported currency.
//! 6. Expiration is valid (not already expired / too soon).
//! 7. No active listing already exists for the token.

use soroban_sdk::{Address, Env};

use super::{listing_storage, types::ListingStatus};
use crate::{
    blacklist, pause_guard, payment_currency, token_owner_storage,
    types::{Error, TokenId},
};

pub const MIN_LISTING_DURATION_SECS: u64 = 3_600;
pub const MAX_LISTING_PRICE: i128 = i128::MAX / 2;

pub fn is_expired(now: u64, expires_at: u64) -> bool {
    expires_at != 0 && expires_at < now.saturating_add(MIN_LISTING_DURATION_SECS)
}

/// Validate all preconditions for creating a new marketplace listing.
///
/// # Arguments
/// * `env`           — Contract execution environment.
/// * `seller`        — Address attempting to create the listing.
/// * `token_id`      — Token to be listed.
/// * `price`         — Asking price in the payment asset's smallest unit.
/// * `payment_asset` — Payment token contract address.
/// * `expires_at`    — Unix expiration timestamp; `0` = never expires.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ContractPaused` | Contract is globally paused. |
/// | `InvalidAddress` | `seller` is blacklisted. |
/// | `UnauthorizedOwnerError` | `seller` is not the token owner. |
/// | `InvalidSalePrice` | `price` ≤ 0. |
/// | `PriceOverflow` | `price` exceeds `MAX_LISTING_PRICE`. |
/// | `UnsupportedAsset` | `payment_asset` is not a registered currency. |
/// | `InvalidConfig` | `expires_at` is already expired or too soon. |
/// | `DuplicateListing` | An active listing already exists for `token_id`. |
pub fn validate_listing(
    env: &Env,
    seller: &Address,
    token_id: TokenId,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
) -> Result<(), Error> {
    // 1. Contract must not be paused.
    pause_guard::require_not_paused(env)?;

    // 2. Seller must not be blacklisted.
    if blacklist::is_blacklisted(env, seller) {
        return Err(Error::InvalidAddress);
    }

    // 3. Seller must own the token.
    token_owner_storage::verify_owner(env, token_id, seller)?;

    // 4. Price must be positive and within bounds.
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    if price > MAX_LISTING_PRICE {
        return Err(Error::PriceOverflow);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if is_expired(env.ledger().timestamp(), expires_at) {
        return Err(Error::InvalidConfig);
    }
    if listing_storage::has_listing(env, token_id) {
        return Err(Error::DuplicateListing);
    }
    Ok(())
}

/// Cancel an active listing after authorization and state checks.
pub fn cancel_listing(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if blacklist::is_blacklisted(env, caller) {
        return Err(Error::InvalidAddress);
    }
    token_owner_storage::verify_owner(env, token_id, caller)?;
    let mut listing = listing_storage::get_listing(env, token_id)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    listing.status = ListingStatus::Cancelled;
    listing_storage::save_listing(env, &listing);
    Ok(())
}
