//! Marketplace & royalty event tests (issue #1367).
//!
//! Verifies the topic and payload of every marketplace and royalty event:
//! listing, listing update, cancellation, sale, offer creation, offer
//! acceptance, and royalty payment.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    vec, Address, Env, IntoVal, String, Symbol, Val, Vec,
};

use crate::event_topics::{TOPIC_LISTING, TOPIC_SALE};
use crate::events::listing::ListingUpdatedEvent;
use crate::marketplace::types::{
    ListingCancelledEvent, NftListedEvent, NftSoldEvent, OfferAcceptedEvent, OfferCreatedEvent,
};
use crate::types::RoyaltyPaidEvent;
use crate::AtomicMintContract;

fn setup() -> (Env, Address) {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    (env, contract_id)
}

/// Asserts exactly one event was emitted by `contract` with the given topic
/// and payload.
fn assert_single_event<T: IntoVal<Env, Val>>(
    env: &Env,
    contract: &Address,
    topic: Symbol,
    data: T,
) {
    let expected: Vec<(Address, Vec<Val>, Val)> = vec![
        env,
        (contract.clone(), (topic,).into_val(env), data.into_val(env)),
    ];
    assert!(env.events().all() == expected);
}

#[test]
fn listing_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::nft_listed_event::emit_nft_listed(&env, 1, 7, &seller, 1_000, &asset, 100);
        assert_single_event(
            &env,
            &id,
            TOPIC_LISTING,
            NftListedEvent {
                listing_id: 1,
                token_id: 7,
                seller,
                price: 1_000,
                asset,
                timestamp: 100,
            },
        );
    });
}

#[test]
fn listing_update_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let seller = Address::generate(&env);
        crate::events::listing::emit_listing_updated(
            &env, 2, 8, &seller, 1_000, 2_000, 0, 500, 200,
        );
        assert_single_event(
            &env,
            &id,
            TOPIC_LISTING,
            ListingUpdatedEvent {
                listing_id: 2,
                token_id: 8,
                seller,
                old_price: 1_000,
                new_price: 2_000,
                old_expires_at: 0,
                new_expires_at: 500,
                timestamp: 200,
            },
        );
    });
}

#[test]
fn listing_cancelled_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let seller = Address::generate(&env);
        crate::listing_cancelled_event::emit_listing_cancelled(&env, 3, 9, &seller, 300);
        assert_single_event(
            &env,
            &id,
            symbol_short!("list_cncl"),
            ListingCancelledEvent {
                listing_id: 3,
                token_id: 9,
                seller,
                timestamp: 300,
            },
        );
    });
}

#[test]
fn sale_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::nft_sold_event::emit_nft_sold(&env, 4, 10, &seller, &buyer, 5_000, &asset, 400);
        assert_single_event(
            &env,
            &id,
            TOPIC_SALE,
            NftSoldEvent {
                listing_id: 4,
                token_id: 10,
                seller,
                buyer,
                sale_amount: 5_000,
                payment_asset: asset,
                timestamp: 400,
            },
        );
    });
}

#[test]
fn offer_created_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::offer_created_event::emit_offer_created(&env, 5, 11, &buyer, 750, &asset, 9_999);
        assert_single_event(
            &env,
            &id,
            symbol_short!("ofr_crt"),
            OfferCreatedEvent {
                offer_id: 5,
                token_id: 11,
                buyer,
                offer_amount: 750,
                asset,
                expiration: 9_999,
            },
        );
    });
}

#[test]
fn offer_accepted_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        crate::offer_accepted_event::emit_offer_accepted(&env, 6, 12, &buyer, &seller, 750, 600);
        assert_single_event(
            &env,
            &id,
            symbol_short!("ofr_acc"),
            OfferAcceptedEvent {
                offer_id: 6,
                token_id: 12,
                buyer,
                seller,
                accepted_amount: 750,
                timestamp: 600,
            },
        );
    });
}

#[test]
fn royalty_paid_event_topic_and_payload() {
    let (env, id) = setup();
    env.as_contract(&id, || {
        let payer = Address::generate(&env);
        let recipient = Address::generate(&env);
        let asset = Address::generate(&env);
        let reference = String::from_str(&env, "offer-6");
        crate::royalty_paid_event::emit_royalty_paid(
            &env,
            12,
            &payer,
            &recipient,
            75,
            &Some(asset.clone()),
            &reference,
            700,
        );
        assert_single_event(
            &env,
            &id,
            symbol_short!("ryl_paid"),
            RoyaltyPaidEvent {
                token_id: 12,
                payer,
                receiver: recipient,
                amount: 75,
                asset_address: Some(asset),
                sale_reference: reference,
                timestamp: 700,
            },
        );
    });
}
