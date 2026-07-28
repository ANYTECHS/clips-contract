//! Integration tests for the CreatorRegistered (creator-assigned) event (issue #696).
//!
//! Verifies that a `"creator"` event is emitted — with the correct creator
//! address, token ID, clip ID, and timestamp — after a creator is successfully
//! associated with a newly minted NFT.

#![cfg(test)]

use clips_nft::{execute_mint, types::CreatorAssignedEvent, MintRequest, Royalty};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Val, Vec,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

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
            recipient: Address::generate(env),
            basis_points: 500,
            asset_address: None,
        },
        creator_address: creator.map(|c| c.clone()),
        creator_display_name: None,
    }
}

fn find_creator_event(env: &Env, token_id: u32) -> Option<CreatorAssignedEvent> {
    for i in 0..env.events().all().events().len() {
        if let Ok((_, evt)) = env
            .events()
            .all()
            .events()
            .get(i)
            .map(|(t, d): (Vec<Val>, CreatorAssignedEvent)| (t, d))
        {
            if evt.token_id == token_id {
                return Some(evt);
            }
        }
    }
    None
}

// ─── tests ────────────────────────────────────────────────────────────────────

/// A successful mint must emit a CreatorAssignedEvent.
#[test]
fn test_creator_registered_event_emitted_on_mint() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let result =
        execute_mint(&env, make_request(&env, &owner, None, 1)).expect("mint ok");

    assert!(
        find_creator_event(&env, result.token_id).is_some(),
        "CreatorAssignedEvent not found for token {}",
        result.token_id
    );
}

/// All four acceptance-criteria fields must match.
#[test]
fn test_creator_registered_event_fields_correct() {
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: 1_720_000_000,
        protocol_version: 21,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 3_110_400,
    });

    let owner = Address::generate(&env);
    let creator = Address::generate(&env);
    let clip_id: u32 = 42;

    let result =
        execute_mint(&env, make_request(&env, &owner, Some(&creator), clip_id)).expect("mint ok");

    let evt =
        find_creator_event(&env, result.token_id).expect("CreatorAssignedEvent not found");

    assert_eq!(evt.token_id, result.token_id, "token_id mismatch");
    assert_eq!(evt.creator, creator, "creator mismatch");
    assert_eq!(evt.clip_id, clip_id, "clip_id mismatch");
    assert_eq!(evt.timestamp, 1_720_000_000, "timestamp should match ledger");
}

/// When no creator_address is supplied the event must use the owner address.
#[test]
fn test_creator_registered_event_falls_back_to_owner() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let result =
        execute_mint(&env, make_request(&env, &owner, None, 2)).expect("mint ok");

    let evt =
        find_creator_event(&env, result.token_id).expect("CreatorAssignedEvent not found");

    assert_eq!(
        evt.creator, owner,
        "creator should default to owner when creator_address is None"
    );
}

/// When an explicit creator_address differs from the owner, the event must
/// carry the creator, not the owner.
#[test]
fn test_creator_registered_event_uses_explicit_creator_address() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let creator = Address::generate(&env);

    let result =
        execute_mint(&env, make_request(&env, &owner, Some(&creator), 3)).expect("mint ok");

    let evt =
        find_creator_event(&env, result.token_id).expect("CreatorAssignedEvent not found");

    assert_eq!(evt.creator, creator, "explicit creator_address should appear in event");
    assert_ne!(evt.creator, owner, "event creator should not be the owner");
}

/// Each token minted in separate calls must produce its own creator event.
#[test]
fn test_creator_registered_event_emitted_per_token() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let c1 = Address::generate(&env);
    let c2 = Address::generate(&env);

    let res1 =
        execute_mint(&env, make_request(&env, &owner, Some(&c1), 10)).expect("first mint ok");
    let res2 =
        execute_mint(&env, make_request(&env, &owner, Some(&c2), 11)).expect("second mint ok");

    let evt1 = find_creator_event(&env, res1.token_id).expect("event for token 1 missing");
    let evt2 = find_creator_event(&env, res2.token_id).expect("event for token 2 missing");

    assert_eq!(evt1.creator, c1);
    assert_eq!(evt2.creator, c2);
    assert_ne!(evt1.token_id, evt2.token_id);
}
