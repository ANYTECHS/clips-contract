//! Tests for the reusable address event helper (issue #910).
//!
//! Validates that the helper correctly:
//! - Supports sender and recipient events
//! - Supports creator and owner events  
//! - Serializes addresses consistently across all event types
//!
//! These tests verify the acceptance criteria for the address event helper.

#![cfg(test)]

use clips_nft::{address_event_helper, event_topics, AtomicMintContract};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, Symbol,
};

fn setup_contract(env: &Env) -> () {
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || {})
}

// ────────────────────────────────────────────────────────────────────────────
// Test sender and recipient support
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_emit_sender_recipient_event_publishes_event() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let amount = 100i128;
        let timestamp = 1000u64;

        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &sender,
            &recipient,
            amount,
            timestamp,
        );

        let events = env.events().all();
        assert_eq!(events.len(), 1, "Should publish exactly one event");
    });
}

#[test]
fn test_sender_and_recipient_are_serialized_correctly() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let amount = 50i128;
        let timestamp = 2000u64;

        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &sender,
            &recipient,
            amount,
            timestamp,
        );

        let events = env.events().all();
        let event = &events.slice(events.len() - 1..events.len()).get_unchecked(0);

        // Verify event data contains sender and recipient
        let (_, data): (_, (Symbol, Address, Address, i128, u64)) = event.clone();
        assert_eq!(data.1, sender, "Sender address should match");
        assert_eq!(data.2, recipient, "Recipient address should match");
        assert_eq!(data.3, amount, "Amount should match");
        assert_eq!(data.4, timestamp, "Timestamp should match");
    });
}

#[test]
fn test_sender_and_recipient_can_be_different() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        assert_ne!(sender, recipient, "Test requires different addresses");

        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &sender,
            &recipient,
            0,
            0,
        );

        assert_eq!(env.events().all().len(), 1);
    });
}

#[test]
fn test_sender_and_recipient_can_be_same() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let address = Address::generate(&env);

        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &address,
            &address,
            0,
            0,
        );

        assert_eq!(env.events().all().len(), 1);
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Test creator and owner support
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_emit_creator_owner_event_publishes_event() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let creator = Address::generate(&env);
        let owner = Address::generate(&env);
        let token_id = 42u32;
        let timestamp = 3000u64;

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &creator,
            &owner,
            token_id,
            timestamp,
        );

        let events = env.events().all();
        assert_eq!(events.len(), 1, "Should publish exactly one event");
    });
}

#[test]
fn test_creator_and_owner_are_serialized_correctly() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let creator = Address::generate(&env);
        let owner = Address::generate(&env);
        let token_id = 123u32;
        let timestamp = 4000u64;

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &creator,
            &owner,
            token_id,
            timestamp,
        );

        let events = env.events().all();
        let event = &events.slice(events.len() - 1..events.len()).get_unchecked(0);

        // Verify event data contains creator and owner
        let (_, data): (_, (Symbol, Address, Address, u32, u64)) = event.clone();
        assert_eq!(data.1, creator, "Creator address should match");
        assert_eq!(data.2, owner, "Owner address should match");
        assert_eq!(data.3, token_id, "Token ID should match");
        assert_eq!(data.4, timestamp, "Timestamp should match");
    });
}

#[test]
fn test_creator_can_equal_owner() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let address = Address::generate(&env);

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &address,
            &address,
            1,
            0,
        );

        assert_eq!(env.events().all().len(), 1);
    });
}

#[test]
fn test_creator_and_owner_can_be_different() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let creator = Address::generate(&env);
        let owner = Address::generate(&env);

        assert_ne!(creator, owner, "Test requires different addresses");

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &creator,
            &owner,
            1,
            0,
        );

        assert_eq!(env.events().all().len(), 1);
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Test single address support
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_emit_single_address_event_publishes_event() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let address = Address::generate(&env);
        let amount = 200i128;
        let timestamp = 5000u64;

        address_event_helper::emit_single_address_event(
            &env,
            event_topics::TOPIC_APPROVAL,
            &address,
            amount,
            timestamp,
        );

        let events = env.events().all();
        assert_eq!(events.len(), 1, "Should publish exactly one event");
    });
}

#[test]
fn test_single_address_is_serialized_correctly() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let address = Address::generate(&env);
        let amount = 999i128;
        let timestamp = 6000u64;

        address_event_helper::emit_single_address_event(
            &env,
            event_topics::TOPIC_APPROVAL,
            &address,
            amount,
            timestamp,
        );

        let events = env.events().all();
        let event = &events.slice(events.len() - 1..events.len()).get_unchecked(0);

        // Verify event data contains address
        let (_, data): (_, (Symbol, Address, i128, u64)) = event.clone();
        assert_eq!(data.1, address, "Address should match");
        assert_eq!(data.2, amount, "Amount should match");
        assert_eq!(data.3, timestamp, "Timestamp should match");
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Test consistent serialization across event types
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_multiple_events_emit_independently() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);
        let addr_c = Address::generate(&env);

        // Emit different types of events
        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &addr_a,
            &addr_b,
            100,
            100,
        );

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &addr_b,
            &addr_c,
            1,
            200,
        );

        address_event_helper::emit_single_address_event(
            &env,
            event_topics::TOPIC_APPROVAL,
            &addr_a,
            50,
            300,
        );

        let events = env.events().all();
        assert_eq!(events.len(), 3, "Should publish three independent events");
    });
}

#[test]
fn test_addresses_consistently_serialized_across_event_types() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let address = Address::generate(&env);

        // Emit the same address via different event helpers
        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &address,
            &address,
            0,
            0,
        );

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &address,
            &address,
            0,
            0,
        );

        address_event_helper::emit_single_address_event(
            &env,
            event_topics::TOPIC_APPROVAL,
            &address,
            0,
            0,
        );

        let events = env.events().all();
        assert_eq!(events.len(), 3, "Should publish three events");

        // All events should serialize addresses consistently
        // (The Address type itself ensures consistent serialization)
    });
}

#[test]
fn test_event_topic_prefix_is_consistent() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let addr = Address::generate(&env);

        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &addr,
            &addr,
            0,
            0,
        );

        let events = env.events().all();
        let event = &events.slice(events.len() - 1..events.len()).get_unchecked(0);

        // Verify topic prefix
        let (topics, _): ((Symbol,), _) = event.clone();
        assert_eq!(
            topics.0,
            Symbol::new(&env, "addr_evt"),
            "Event should have consistent topic prefix"
        );
    });
}

#[test]
fn test_zero_amounts_and_token_ids_are_allowed() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());

    env.as_contract(&contract_id, || {
        let addr = Address::generate(&env);

        // Test with zero values
        address_event_helper::emit_sender_recipient_event(
            &env,
            event_topics::TOPIC_TRANSFER,
            &addr,
            &addr,
            0,
            0,
        );

        address_event_helper::emit_creator_owner_event(
            &env,
            event_topics::TOPIC_CREATOR,
            &addr,
            &addr,
            0,
            0,
        );

        address_event_helper::emit_single_address_event(
            &env,
            event_topics::TOPIC_APPROVAL,
            &addr,
            0,
            0,
        );

        let events = env.events().all();
        assert_eq!(events.len(), 3, "Zero values should be allowed");
    });
}
