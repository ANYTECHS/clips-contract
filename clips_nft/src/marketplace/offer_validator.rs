//! Offer creation pre-conditions (issue #1240).
//!
//! Single place where "may this buyer open this offer" is decided.
//! `make_offer` delegates here so the entry point only handles auth,
//! persistence, and events.

use soroban_sdk::{Address, Env};

use super::{
    offer_storage::has_offer,
    types::{Offer, MAX_MARKETPLACE_PRICE},
};
use crate::{
    blacklist, frozen_token, pause_guard, payment_currency, token_owner_storage, token_storage,
    types::{Error, TokenId},
};

/// Validate an offer request without writing storage.
///
/// Check order mirrors `make_offer`: pause → frozen → blacklist → price
/// bounds → asset → token existence → duplicate → expiry.
pub fn validate_offer(
    env: &Env,
    buyer: &Address,
    token_id: TokenId,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }
    if blacklist::is_blacklisted(env, buyer) {
        return Err(Error::InvalidAddress);
    }
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    if price > MAX_MARKETPLACE_PRICE {
        return Err(Error::PriceOverflow);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    token_storage::require_token_exists(env, token_id)?;
    if has_offer(env, token_id) {
        return Err(Error::OfferAlreadyExists);
    }
    let now = env.ledger().timestamp();
    if expires_at != 0 && expires_at <= now {
        return Err(Error::OfferExpired);
    }
    Ok(())
}

/// Build the [`Offer`] record for a validated request. Callers run
/// [`validate_offer`] first.
pub fn build_offer(
    env: &Env,
    buyer: &Address,
    token_id: TokenId,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
) -> Offer {
    Offer {
        offer_id: token_id as u64,
        token_id,
        buyer: buyer.clone(),
        price,
        payment_asset: payment_asset.clone(),
        expires_at,
        status: super::types::OfferStatus::Active,
        created_at: env.ledger().timestamp(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        crate::token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    fn setup_asset(env: &Env) -> Address {
        let asset = Address::generate(env);
        crate::payment_currency::add_currency(env, asset.clone()).unwrap();
        asset
    }

    #[test]
    fn valid_offer_passes() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            assert!(validate_offer(env, &buyer, 1, 1000, &asset, 0).is_ok());
        });
    }

    #[test]
    fn rejected_when_paused() {
        with_contract(|env| {
            crate::pause_state::save_pause_state(env, true);
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            assert_eq!(
                validate_offer(env, &buyer, 1, 1000, &asset, 0),
                Err(Error::ContractPaused)
            );
        });
    }

    #[test]
    fn rejected_when_buyer_blacklisted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            crate::blacklist::add_wallet(env, &buyer);
            assert_eq!(
                validate_offer(env, &buyer, 1, 1000, &asset, 0),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn rejected_when_price_not_positive() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            assert_eq!(
                validate_offer(env, &buyer, 1, 0, &asset, 0),
                Err(Error::InvalidSalePrice)
            );
        });
    }

    #[test]
    fn rejected_when_unsupported_asset() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = Address::generate(env);
            assert_eq!(
                validate_offer(env, &buyer, 1, 1000, &asset, 0),
                Err(Error::UnsupportedAsset)
            );
        });
    }

    #[test]
    fn rejected_when_duplicate_offer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            super::super::offer_storage::save_offer(
                env,
                &build_offer(env, &buyer, 1, 500, &asset, 0),
            );
            assert_eq!(
                validate_offer(env, &buyer, 1, 1000, &asset, 0),
                Err(Error::OfferAlreadyExists)
            );
        });
    }

    #[test]
    fn rejected_when_expired() {
        with_contract(|env| {
            env.ledger().set_timestamp(1_700_000_000);
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            assert_eq!(
                validate_offer(env, &buyer, 1, 1000, &asset, 1_699_000_000),
                Err(Error::OfferExpired)
            );
        });
    }
}
