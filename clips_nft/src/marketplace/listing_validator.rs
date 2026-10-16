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
        return Err(Error::DuplicateListing);
    }

    Ok(())
}
