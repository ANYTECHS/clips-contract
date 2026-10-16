//! Listing creation pre-conditions.
//!
//! Single place where "may this token be listed at this price in this asset
//! with this expiry" is decided, so `list_nft` and tests share one rule set.

use soroban_sdk::{Address, Env};

use super::listing_storage::has_active_listing;
use crate::{
    blacklist, pause_guard, payment_currency, token_owner_storage,
    types::{Error, TokenId},
};

/// Minimum lifetime of a listing in seconds. Anything expiring sooner is
/// malformed at creation time (`Error::InvalidConfig`); `0` means no expiry.
pub const MIN_LISTING_DURATION_SECS: u64 = 3_600;

/// True when `expires_at` names a moment that has passed or is too soon to
/// settle reliably. `0` means "never expires".
pub fn is_expired(now: u64, expires_at: u64) -> bool {
    expires_at != 0 && expires_at < now.saturating_add(MIN_LISTING_DURATION_SECS)
}

/// Validate a listing request without writing storage.
pub fn validate_listing(
    env: &Env,
    seller: &Address,
    token_id: TokenId,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    token_owner_storage::verify_owner(env, token_id, seller)?;
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    if has_active_listing(env, token_id) {
        return Err(Error::DuplicateRecord);
    }
    if is_expired(env.ledger().timestamp(), expires_at) {
        return Err(Error::InvalidConfig);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if blacklist::is_blacklisted(env, seller) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
