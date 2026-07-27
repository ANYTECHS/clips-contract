//! Integration tests for the RoyaltyAssigned event (issue #695).
//!
//! Verifies that a `"rylty_asgn"` event is emitted — with the correct
//! token ID, royalty recipient, basis points, and timestamp — every time a
//! successful mint persists royalty information.

#![cfg(test)]

use clips_nft::{
    mint_service::execute_mint, types::RoyaltyAssignedEvent, MintRequest, Royalty,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, LedgerInfo},
    symbol_short, Address, Env, String, Val, Vec,
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn make_request(env: &Env, owner: &Address, recipient: &Address, clip_id: u32, bps: u32) -> MintRequest {
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, &format!("ipfs://QmClip{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient: recipient.clone(),
            basis_points: bps,
            asset_address: None,
        },
    }
}

// ─── Event emission ───────────────────────────────────────────────────────────

/// After a successful mint the event list must contain a `"rylty_asgn"` entry.
#[test]
fn test_royalty_assigned_event_emitted_on_mint() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = execute_mint(&env, make_request(&env, &owner, &recipient, 1, 500))
        .expect("mint should succeed");

    let all = env.events().all();
    // At minimum the royalty-assigned event should be present (there will also
    // be the mint event, but we only care that ours is there).
    let royalty_events: Vec<(Vec<Val>, RoyaltyAssignedEvent)> = all
        .events()
        .iter()
        .filter_map(|(topics, data): (Vec<Val>, RoyaltyAssignedEvent)| {
            Some((topics, data))
        })
        .filter(|(_, data)| data.token_id == result.token_id)
        .collect();

    assert!(
        !royalty_events.is_empty(),
        "expected at least one RoyaltyAssignedEvent for token {}",
        result.token_id
    );
}

/// The event fields must exactly match the values supplied in the mint request.
#[test]
fn test_royalty_assigned_event_fields_are_correct() {
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
    let recipient = Address::generate(&env);
    let basis_points: u32 = 750;

    let result = execute_mint(&env, make_request(&env, &owner, &recipient, 2, basis_points))
        .expect("mint should succeed");

    // Find the RoyaltyAssignedEvent for this token among all published events.
    let mut found: Option<RoyaltyAssignedEvent> = None;
    for i in 0..env.events().all().events().len() {
        if let Ok((_, evt)) =
            env.events()
                .all()
                .events()
                .get(i)
                .map(|(t, d): (Vec<Val>, RoyaltyAssignedEvent)| (t, d))
        {
            if evt.token_id == result.token_id {
                found = Some(evt);
                break;
            }
        }
    }

    let evt = found.expect("RoyaltyAssignedEvent not found in emitted events");
    assert_eq!(evt.token_id, result.token_id, "token_id mismatch");
    assert_eq!(evt.recipient, recipient, "recipient mismatch");
    assert_eq!(evt.basis_points, basis_points, "basis_points mismatch");
    assert_eq!(evt.timestamp, 1_720_000_000, "timestamp mismatch");
}

/// Zero basis points (no royalty) should still emit the event so indexers can
/// record the explicit royalty-free assignment.
#[test]
fn test_royalty_assigned_event_emitted_for_zero_bps() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = execute_mint(&env, make_request(&env, &owner, &recipient, 3, 0))
        .expect("mint with zero bps should succeed");

    // At least one event must be present and one of them must be for this token.
    assert!(
        env.events().all().events().len() > 0,
        "no events emitted at all"
    );

    let found = env
        .events()
        .all()
        .events()
        .iter()
        .filter_map(|(_, data): (Vec<Val>, RoyaltyAssignedEvent)| Some(data))
        .any(|e| e.token_id == result.token_id && e.basis_points == 0);

    assert!(found, "expected RoyaltyAssignedEvent with basis_points=0");
}

/// Each token minted in separate calls must emit its own event, each with the
/// correct token_id.
#[test]
fn test_royalty_assigned_event_emitted_per_mint() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);

    let res1 = execute_mint(&env, make_request(&env, &owner, &r1, 10, 500))
        .expect("first mint ok");
    let res2 = execute_mint(&env, make_request(&env, &owner, &r2, 11, 1000))
        .expect("second mint ok");

    let token_ids_in_events: Vec<u32> = env
        .events()
        .all()
        .events()
        .iter()
        .filter_map(|(_, data): (Vec<Val>, RoyaltyAssignedEvent)| Some(data.token_id))
        .collect();

    assert!(
        token_ids_in_events.contains(&res1.token_id),
        "no event for first minted token {}",
        res1.token_id
    );
    assert!(
        token_ids_in_events.contains(&res2.token_id),
        "no event for second minted token {}",
        res2.token_id
    );
}

/// The timestamp on the event must reflect the ledger time at the point of minting.
#[test]
fn test_royalty_assigned_event_timestamp_matches_ledger() {
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: 9_999_999,
        protocol_version: 21,
        sequence_number: 5,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 3_110_400,
    });

    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = execute_mint(&env, make_request(&env, &owner, &recipient, 20, 300))
        .expect("mint ok");

    let evt: RoyaltyAssignedEvent = env
        .events()
        .all()
        .events()
        .iter()
        .filter_map(|(_, d): (Vec<Val>, RoyaltyAssignedEvent)| Some(d))
        .find(|e| e.token_id == result.token_id)
        .expect("RoyaltyAssignedEvent not found");

    assert_eq!(evt.timestamp, 9_999_999, "event timestamp must match ledger timestamp");
}
