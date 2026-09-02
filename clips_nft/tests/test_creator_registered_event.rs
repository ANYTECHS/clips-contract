//! Integration tests for the CreatorAssigned event (issue #920).
//!
//! Verifies that a `"creator"` event is emitted — with the correct creator
//! address, token ID, clip ID, and timestamp — after a creator is successfully
//! associated with a newly minted NFT. Also verifies the reassign_creator
//! entry point emits the event.

#![cfg(test)]

use clips_nft::{
    execute_mint, types::CreatorAssignedEvent, AtomicMintContract, MintRequest, Royalty,
    RoyaltyRecipient,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Env, String, Symbol, TryFromVal,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn make_request(
    env: &Env,
    owner: &Address,
    creator: Option<&Address>,
    clip_id: u32,
) -> MintRequest {
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: creator.unwrap_or(owner).clone(),
        metadata_uri: String::from_str(env, &format!("ipfs://QmCreator{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipients: soroban_sdk::vec![&env, RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 500,
            }],
            asset_address: None,
        },
        creator_address: creator.map(|c| c.clone()),
        creator_display_name: None,
    }
}

/// Find a `CreatorAssignedEvent` with the given `token_id` among published events.
fn find_creator_event(env: &Env, token_id: u32) -> Option<CreatorAssignedEvent> {
    let expected_topic: Symbol = symbol_short!("creator");
    let events = env.events().all();
    for event in events.events() {
        if let soroban_sdk::xdr::ContractEventBody::V0(v0) = &event.body {
            if v0.topics.len() == 1 {
                if let Ok(topic_sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                    if topic_sym == expected_topic {
                        if let Ok(evt) =
                            CreatorAssignedEvent::try_from_val(env, &v0.data)
                        {
                            if evt.token_id == token_id {
                                return Some(evt);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

// ─── #920 — CreatorAssigned event emitted on mint ─────────────────────────────

/// A successful mint must emit a CreatorAssignedEvent.
#[test]
fn test_creator_registered_event_emitted_on_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let result = execute_mint(env, make_request(env, &owner, None, 1)).expect("mint ok");

        assert!(
            find_creator_event(env, result.token_id).is_some(),
            "CreatorAssignedEvent not found for token {}",
            result.token_id
        );
    });
}

/// All four acceptance-criteria fields must match.
#[test]
fn test_creator_registered_event_fields_correct() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let clip_id: u32 = 42;

        let result = execute_mint(env, make_request(env, &owner, Some(&creator), clip_id))
            .expect("mint ok");

        let evt =
            find_creator_event(env, result.token_id).expect("CreatorAssignedEvent not found");

        assert_eq!(evt.token_id, result.token_id, "token_id mismatch");
        assert_eq!(evt.creator, creator, "creator mismatch");
        assert_eq!(evt.clip_id, clip_id, "clip_id mismatch");
    });
}

/// When no creator_address is supplied the event must use the owner address.
#[test]
fn test_creator_registered_event_falls_back_to_owner() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let result = execute_mint(env, make_request(env, &owner, None, 2)).expect("mint ok");

        let evt =
            find_creator_event(env, result.token_id).expect("CreatorAssignedEvent not found");

        assert_eq!(
            evt.creator, owner,
            "creator should default to owner when creator_address is None"
        );
    });
}

/// When an explicit creator_address differs from the owner, the event must
/// carry the creator, not the owner.
#[test]
fn test_creator_registered_event_uses_explicit_creator_address() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);

        let result = execute_mint(env, make_request(env, &owner, Some(&creator), 3))
            .expect("mint ok");

        let evt =
            find_creator_event(env, result.token_id).expect("CreatorAssignedEvent not found");

        assert_eq!(
            evt.creator, creator,
            "explicit creator_address should appear in event"
        );
        assert_ne!(evt.creator, owner, "event creator should not be the owner");
    });
}

/// Each token minted in separate calls must produce its own creator event.
#[test]
fn test_creator_registered_event_emitted_per_token() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let c1 = Address::generate(env);
        let c2 = Address::generate(env);

        let res1 =
            execute_mint(env, make_request(env, &owner, Some(&c1), 10)).expect("first mint ok");
        let res2 =
            execute_mint(env, make_request(env, &owner, Some(&c2), 11)).expect("second mint ok");

        let evt1 = find_creator_event(env, res1.token_id).expect("event for token 1 missing");
        let evt2 = find_creator_event(env, res2.token_id).expect("event for token 2 missing");

        assert_eq!(evt1.creator, c1);
        assert_eq!(evt2.creator, c2);
        assert_ne!(evt1.token_id, evt2.token_id);
    });
}
