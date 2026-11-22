//! NFT lifecycle event tests (issue #1359).
//!
//! Verifies topics and payloads for mint, transfer, burn, freeze, unfreeze,
//! metadata update and creator assignment events.

use clips_nft::event_topics::{
    TOPIC_BURN, TOPIC_FREEZE, TOPIC_MINT, TOPIC_TRANSFER, TOPIC_UNFREEZE,
};
use clips_nft::types::{
    CreatorAssignedEvent, MetadataUpdatedEvent, NFTBurnedEvent, NFTFrozenEvent, NFTMintedEvent,
    NFTTransferredEvent, NFTUnfrozenEvent,
};
use clips_nft::{
    burn_event, creator_event, metadata_updated_event, mint_event, nft_frozen_event,
    nft_unfrozen_event, transfer_event,
};
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::xdr::ContractEventBody;
use soroban_sdk::{contract, symbol_short, Address, Env, String, Symbol, TryFromVal, Val};

#[contract]
struct Dummy;

const TS: u64 = 1_720_000_000;

/// Run `f` inside a contract context and return the single emitted event's
/// first topic and decoded payload.
fn emit_one<T: TryFromVal<Env, Val>>(f: impl FnOnce(&Env)) -> (Env, Symbol, T) {
    let env = Env::default();
    let id = env.register(Dummy, ());
    env.as_contract(&id, || f(&env));
    let all = env.events().all();
    let events = all.events();
    assert_eq!(events.len(), 1, "exactly one event expected");
    let ContractEventBody::V0(body) = &events[0].body;
    let topic_val = Val::try_from_val(&env, &body.topics[0]).unwrap();
    let topic = Symbol::try_from_val(&env, &topic_val).unwrap();
    let data_val = Val::try_from_val(&env, &body.data).unwrap();
    let data = T::try_from_val(&env, &data_val).expect("payload should decode");
    (env, topic, data)
}

#[test]
fn mint_event_topic_and_payload() {
    let (env, topic, data): (_, _, NFTMintedEvent) = emit_one(|env| {
        let a = Address::generate(env);
        mint_event::emit_nft_minted(env, 1, 10, &a, &a, &String::from_str(env, "ipfs://m"), TS);
    });
    assert_eq!(topic, TOPIC_MINT);
    assert_eq!(data.token_id, 1);
    assert_eq!(data.clip_id, 10);
    assert_eq!(data.creator, data.owner);
    assert_eq!(data.metadata_uri, String::from_str(&env, "ipfs://m"));
    assert_eq!(data.timestamp, TS);
}

#[test]
fn transfer_event_topic_and_payload() {
    let (_env, topic, data): (_, _, NFTTransferredEvent) = emit_one(|env| {
        let from = Address::generate(env);
        let to = Address::generate(env);
        transfer_event::emit_nft_transferred(env, 2, &from, &to, TS);
    });
    assert_eq!(topic, TOPIC_TRANSFER);
    assert_eq!(data.token_id, 2);
    assert_ne!(data.previous_owner, data.new_owner);
    assert_eq!(data.timestamp, TS);
}

#[test]
fn burn_event_topic_and_payload() {
    let (_env, topic, data): (_, _, NFTBurnedEvent) = emit_one(|env| {
        let owner = Address::generate(env);
        burn_event::emit_nft_burned(env, 3, &owner, &owner, TS);
    });
    assert_eq!(topic, TOPIC_BURN);
    assert_eq!(data.token_id, 3);
    assert_eq!(data.previous_owner, data.caller);
    assert_eq!(data.timestamp, TS);
}

#[test]
fn freeze_event_topic_and_payload() {
    let (env, topic, data): (_, _, NFTFrozenEvent) = emit_one(|env| {
        let caller = Address::generate(env);
        let reason = String::from_str(env, "dispute");
        nft_frozen_event::emit_nft_frozen(env, 4, &caller, Some(&reason), TS);
    });
    assert_eq!(topic, TOPIC_FREEZE);
    assert_eq!(data.token_id, 4);
    assert_eq!(data.reason, Some(String::from_str(&env, "dispute")));
    assert_eq!(data.timestamp, TS);
}

#[test]
fn unfreeze_event_topic_and_payload() {
    let (_env, topic, data): (_, _, NFTUnfrozenEvent) = emit_one(|env| {
        let caller = Address::generate(env);
        nft_unfrozen_event::emit_nft_unfrozen(env, 5, &caller, TS);
    });
    assert_eq!(topic, TOPIC_UNFREEZE);
    assert_eq!(data.token_id, 5);
    assert_eq!(data.timestamp, TS);
}

#[test]
fn metadata_update_event_topic_and_payload() {
    let (env, topic, data): (_, _, MetadataUpdatedEvent) = emit_one(|env| {
        let caller = Address::generate(env);
        metadata_updated_event::emit_metadata_updated(
            env,
            6,
            &String::from_str(env, "ipfs://old"),
            &String::from_str(env, "ipfs://new"),
            &caller,
            TS,
        );
    });
    assert_eq!(topic, symbol_short!("meta_upd"));
    assert_eq!(data.token_id, 6);
    assert_eq!(data.previous_uri, String::from_str(&env, "ipfs://old"));
    assert_eq!(data.new_uri, String::from_str(&env, "ipfs://new"));
    assert_eq!(data.timestamp, TS);
}

#[test]
fn creator_assignment_event_topic_and_payload() {
    let (_env, topic, data): (_, _, CreatorAssignedEvent) = emit_one(|env| {
        let creator = Address::generate(env);
        creator_event::emit_creator_assigned(env, 7, &creator, 70, TS);
    });
    assert_eq!(topic, symbol_short!("creator"));
    assert_eq!(data.token_id, 7);
    assert_eq!(data.clip_id, 70);
    assert_eq!(data.timestamp, TS);
}
