//! Offer acceptance settlement (issue #1241).
//!
//! Single place where "may this seller accept this offer, and how are funds
//! routed" is decided. `accept_offer` delegates here so the entry point only
//! handles auth. Steps: verify offer → ownership → royalty → platform fee →
//! seller payout → NFT transfer → mark accepted → events.

use soroban_sdk::{token, Address, Env};

use super::{
    offer_storage,
    types::{Offer, OfferStatus},
};
use crate::{
    blacklist, frozen_token, offer_accepted_event, pause_guard, payment_currency, royalty_payment,
    token_owner_storage,
    types::{Error, TokenId},
};

/// Verify the offer may be accepted by `seller`. Returns the offer for
/// settlement.
///
/// Check order: pause → frozen → load → blacklist → status → expiry →
/// ownership → self-purchase → asset.
pub fn validate_acceptance(env: &Env, seller: &Address, token_id: TokenId) -> Result<Offer, Error> {
    pause_guard::require_not_paused(env)?;
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }
    let offer = offer_storage::get_offer(env, token_id)?;
    if blacklist::is_blacklisted(env, seller) || blacklist::is_blacklisted(env, &offer.buyer) {
        return Err(Error::InvalidAddress);
    }
    if offer.status != OfferStatus::Active {
        return Err(Error::OfferExpired);
    }
    if offer.expires_at != 0 && offer.expires_at <= env.ledger().timestamp() {
        return Err(Error::OfferExpired);
    }
    token_owner_storage::verify_owner(env, token_id, seller)?;
    if *seller == offer.buyer {
        return Err(Error::SelfTransferNotAllowed);
    }
    if !payment_currency::is_supported(env, &offer.payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    Ok(offer)
}

/// Seller proceeds after royalty and platform fee: `price - total_royalty -
/// platform_fee`. Underflow → `Error::InvalidSalePrice`.
pub fn compute_seller_net(
    price: i128,
    total_royalty: i128,
    platform_fee: i128,
) -> Result<i128, Error> {
    price
        .checked_sub(total_royalty)
        .and_then(|v| v.checked_sub(platform_fee))
        .ok_or(Error::InvalidSalePrice)
}

/// Route funds, transfer the NFT, close the offer, and emit events.
/// Callers run [`validate_acceptance`] first.
pub fn settle_acceptance(env: &Env, seller: &Address, offer: &Offer) -> Result<(), Error> {
    let result = royalty_payment::pay_royalty(env, &offer.buyer, offer.token_id, offer.price)?;

    let token_client = token::Client::new(env, &offer.payment_asset);
    if result.platform_fee > 0 {
        token_client.transfer(
            &offer.buyer,
            &env.current_contract_address(),
            &result.platform_fee,
        );
    }
    let seller_net = compute_seller_net(offer.price, result.total_royalty, result.platform_fee)?;
    if seller_net > 0 {
        token_client.transfer(&offer.buyer, seller, &seller_net);
    }

    token_owner_storage::update_owner(env, offer.token_id, &offer.buyer)?;
    let mut completed = offer.clone();
    completed.status = OfferStatus::Accepted;
    offer_storage::update_offer(env, &completed)?;
    offer_storage::remove_offer(env, offer.token_id);

    let now = env.ledger().timestamp();
    offer_accepted_event::emit_offer_accepted(
        env,
        completed.offer_id,
        completed.token_id,
        &completed.buyer,
        seller,
        completed.price,
        now,
    );
    crate::transfer_event::emit_nft_transferred(env, offer.token_id, seller, &completed.buyer, now);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::listing_validator::is_expired;
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
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    fn setup_offer(env: &Env, buyer: &Address, asset: &Address) {
        crate::payment_currency::add_currency(env, asset.clone()).unwrap();
        let offer = Offer {
            offer_id: 1,
            token_id: 1,
            buyer: buyer.clone(),
            price: 1000,
            payment_asset: asset.clone(),
            expires_at: 0,
            status: OfferStatus::Active,
            created_at: 0,
        };
        offer_storage::save_offer(env, &offer);
    }

    #[test]
    fn valid_acceptance_returns_offer() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &seller);
            let asset = Address::generate(env);
            setup_offer(env, &buyer, &asset);
            let offer = validate_acceptance(env, &seller, 1).unwrap();
            assert_eq!(offer.buyer, buyer);
            assert_eq!(offer.price, 1000);
        });
    }

    #[test]
    fn rejected_when_paused() {
        with_contract(|env| {
            crate::pause_state::save_pause_state(env, true);
            let seller = Address::generate(env);
            assert_eq!(
                validate_acceptance(env, &seller, 1),
                Err(Error::ContractPaused)
            );
        });
    }

    #[test]
    fn rejected_when_no_offer() {
        with_contract(|env| {
            let seller = Address::generate(env);
            setup_token(env, 1, &seller);
            assert_eq!(
                validate_acceptance(env, &seller, 1),
                Err(Error::OfferExpired)
            );
        });
    }

    #[test]
    fn rejected_when_not_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = Address::generate(env);
            setup_offer(env, &buyer, &asset);
            assert_eq!(
                validate_acceptance(env, &seller, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn rejected_when_seller_is_buyer() {
        with_contract(|env| {
            let seller = Address::generate(env);
            setup_token(env, 1, &seller);
            let asset = Address::generate(env);
            setup_offer(env, &seller, &asset);
            assert_eq!(
                validate_acceptance(env, &seller, 1),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn rejected_when_expired() {
        with_contract(|env| {
            env.ledger().set_timestamp(1_700_000_000);
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &seller);
            let asset = Address::generate(env);
            crate::payment_currency::add_currency(env, asset.clone()).unwrap();
            let offer = Offer {
                offer_id: 1,
                token_id: 1,
                buyer: buyer.clone(),
                price: 1000,
                payment_asset: asset.clone(),
                expires_at: 1_699_000_000,
                status: OfferStatus::Active,
                created_at: 0,
            };
            offer_storage::save_offer(env, &offer);
            assert_eq!(
                validate_acceptance(env, &seller, 1),
                Err(Error::OfferExpired)
            );
        });
    }

    #[test]
    fn seller_net_math_and_underflow() {
        assert_eq!(compute_seller_net(1000, 100, 25), Ok(875));
        assert_eq!(compute_seller_net(1000, 0, 0), Ok(1000));
        assert_eq!(
            compute_seller_net(100, 90, 20),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn is_expired_shared_with_listing_validator() {
        assert!(!is_expired(1_700_000_000, 0));
        assert!(is_expired(1_700_000_000, 1_699_000_000));
        assert!(!is_expired(1_700_000_000, 1_700_000_000 + 3_600));
    }
}
