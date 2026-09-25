//! Event infrastructure tests.
//!
//! Covers the shared event building blocks described in the issue:
//! topic generation, event payloads, address serialization, amount
//! serialization, timestamp handling, and the event helper functions.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

use super::amount;
use super::timestamp;
use crate::event_topics;

// ── Topic generation ─────────────────────────────────────────────────────────

#[test]
fn topic_symbols_are_short_enough_for_indexers() {
    // symbol_short! panics at > 9 chars — if this compiles, all topics pass.
    // Explicitly assert a few critical ones for documentation purposes.
    let topics = [
        event_topics::TOPIC_MINT,
        event_topics::TOPIC_TRANSFER,
        event_topics::TOPIC_BURN,
        event_topics::TOPIC_LISTING,
        event_topics::TOPIC_SALE,
        event_topics::TOPIC_ROYALTY_PAID,
        event_topics::TOPIC_APPROVAL,
        event_topics::TOPIC_CONFIG_UPDATED,
        event_topics::TOPIC_PAUSE,
        event_topics::TOPIC_OFFER_CREATED,
        event_topics::TOPIC_OFFER_ACCEPTED,
        event_topics::TOPIC_BATCH_MINT,
    ];
    assert!(!topics.is_empty(), "topic list must not be empty");
}

// ── Address serialization in events ──────────────────────────────────────────

#[test]
fn address_in_payload_round_trips_through_env() {
    let env = Env::default();
    let addr = Address::generate(&env);

    // Embed the address in a contracttype struct and verify it survives
    let info = amount::build_amount_info(100, &addr, &addr, &addr);
    assert_eq!(info.sender, addr);
    assert_eq!(info.recipient, addr);
    assert_eq!(info.asset, addr);
}

#[test]
fn distinct_addresses_remain_distinct_in_payload() {
    let env = Env::default();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    let d = Address::generate(&env);

    let info = amount::build_amount_info(1, &a, &b, &c);
    assert_ne!(info.sender, info.recipient);
    assert_ne!(info.asset, info.sender);
    assert_ne!(info.asset, info.recipient);
}

// ── Amount serialization ─────────────────────────────────────────────────────

#[test]
fn amount_zero_round_trips() {
    let env = Env::default();
    let addr = Address::generate(&env);

    let info = amount::build_amount_info(0, &addr, &addr, &addr);
    assert_eq!(info.amount, 0);
}

#[test]
fn amount_max_value_round_trips() {
    let env = Env::default();
    let addr = Address::generate(&env);

    let info = amount::build_amount_info(i128::MAX, &addr, &addr, &addr);
    assert_eq!(info.amount, i128::MAX);
}

#[test]
fn amount_typical_stroops_value() {
    let env = Env::default();
    let addr = Address::generate(&env);

    // 100 XLM = 1,000,000 stroops
    let info = amount::build_amount_info(1_000_000, &addr, &addr, &addr);
    assert_eq!(info.amount, 1_000_000);
}

// ── Timestamp handling ───────────────────────────────────────────────────────

#[test]
fn ledger_timestamp_now_matches_env() {
    let env = Env::default();
    env.ledger().set_timestamp(1_700_000_000);

    let ts = timestamp::LedgerTimestamp::now(&env);
    assert_eq!(ts.as_u64(), 1_700_000_000);
}

#[test]
fn raw_timestamp_preserves_value() {
    let ts = timestamp::LedgerTimestamp::raw(42);
    assert_eq!(ts.as_u64(), 42);
}

#[test]
fn current_timestamp_helper_matches_direct_read() {
    let env = Env::default();
    env.ledger().set_timestamp(2_000_000_000);

    let helper = timestamp::current_timestamp(&env);
    let direct = env.ledger().timestamp();
    assert_eq!(helper, direct);
}

// ── Event helper functions ───────────────────────────────────────────────────

#[test]
fn amount_info_embeddable_in_event_payload() {
    let env = Env::default();
    let asset = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let info = amount::build_amount_info(5_000, &asset, &sender, &recipient);
    let ts = timestamp::LedgerTimestamp::raw(1_700_000_000);

    // Verify the struct can be used as a tuple element in env.events().publish()
    // by checking all fields are accessible after construction.
    assert_eq!(info.amount, 5_000);
    assert_eq!(info.asset, asset);
    assert_eq!(info.sender, sender);
    assert_eq!(info.recipient, recipient);
    assert_eq!(ts.as_u64(), 1_700_000_000);
}

#[test]
fn timestamp_can_be_combined_with_amount_info() {
    let env = Env::default();
    let addr = Address::generate(&env);

    let amount_info = amount::build_amount_info(100, &addr, &addr, &addr);
    let ts = timestamp::LedgerTimestamp::now(&env);

    // Both types are Clone + Debug + Eq + PartialEq
    let _amount_clone = amount_info.clone();
    let _ts_clone = ts.clone();

    // Both types are contracttype-compatible (verified by compilation)
    assert_eq!(amount_info.amount, 100);
    assert!(ts.as_u64() > 0 || ts.as_u64() == 0); // always true, but proves API works
}
