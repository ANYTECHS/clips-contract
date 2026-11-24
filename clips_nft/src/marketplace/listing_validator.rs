//! Marketplace listing validation.

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

pub fn validate_listing(
    env: &Env,
    seller: &Address,
    token_id: TokenId,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if blacklist::is_blacklisted(env, seller) {
        return Err(Error::InvalidAddress);
    }
    token_owner_storage::verify_owner(env, token_id, seller)?;
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
        return Err(Error::ListingAlreadyExists);
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
