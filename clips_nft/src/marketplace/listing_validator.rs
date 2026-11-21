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
//! Marketplace listing validator.
//!
//! Checks all preconditions before a new listing is accepted.
//!
//! # Validation order
//! 1. Contract is not paused.
//! 2. Caller is the token owner.
//! 3. Price is positive and within `MAX_LISTING_PRICE`.
//! 4. Payment asset is a supported currency.
//! 5. No active listing already exists for the token.

use soroban_sdk::{Address, Env};

use crate::marketplace::listing_storage;
use crate::pause_state;
use crate::payment_currency;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// Maximum allowed listing price (mirrors `MAX_LISTING_PRICE` in `lib.rs`).
const MAX_LISTING_PRICE: i128 = i128::MAX / 2;

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
/// | `Unauthorized`   | `seller` is not the token owner. |
/// | `InvalidSalePrice` | `price` ≤ 0 or exceeds `MAX_LISTING_PRICE`. |
/// | `UnsupportedAsset` | `payment_asset` is not a registered currency. |
/// | `DuplicateListing` | An active listing already exists for `token_id`. |
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
        return Err(Error::ListingAlreadyExists);
    }
    if is_expired(env.ledger().timestamp(), expires_at) {
        return Err(Error::ListingExpired);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if blacklist::is_blacklisted(env, seller) {
        return Err(Error::Unauthorized);
    }
    // 1. Contract must not be paused.
    if pause_state::get_pause_state(env) {
        return Err(Error::ContractPaused);
    }

    // 2. Seller must own the token.
    let owner = token_owner_storage::get_owner(env, token_id)?;
    if &owner != seller {
        return Err(Error::Unauthorized);
    }

    // 3. Price must be positive and within bounds.
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    if price > MAX_LISTING_PRICE {
        return Err(Error::PriceOverflow);
    }

    // 4. Payment asset must be a supported currency.
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }

    // 5. No duplicate active listing.
    if listing_storage::has_listing(env, token_id) {
        return Err(Error::ListingAlreadyExists);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(crate::contract::Contract, ());
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        (env, contract, seller, asset)
    }

    #[test]
    fn duplicate_active_listing_returns_listing_already_exists() {
        let (env, contract, seller, asset) = setup();
        let token_id: TokenId = 1;
        env.as_contract(&contract, || {
            token_owner_storage::set_owner(&env, token_id, &seller);
            listing_storage::store_listing(&env, token_id, &seller, 100, &asset, 0);
            let result = validate_listing(&env, &seller, token_id, 100, &asset, 0);
            assert_eq!(result, Err(Error::ListingAlreadyExists));
        });
    }

    #[test]
    fn expired_listing_returns_listing_expired() {
        let (env, contract, seller, asset) = setup();
        let token_id: TokenId = 2;
        env.as_contract(&contract, || {
            token_owner_storage::set_owner(&env, token_id, &seller);
            let now = env.ledger().timestamp();
            let result = validate_listing(&env, &seller, token_id, 100, &asset, now + 1);
            assert_eq!(result, Err(Error::ListingExpired));
        });
    }
}
