//! Integration tests for the BatchMintCompleted event (issue #697).
//!
//! Verifies that a `"btch_done"` event is emitted — with the correct
//! batch ID, minted count, recipient, and timestamp — after every fully
//! successful `execute_batch_mint` call.

#![cfg(test)]

use clips_nft::{
    execute_batch_mint, types::BatchMintCompletedEvent, AtomicMintContract, BatchMintRequest,
    MintRequest, Royalty,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Val, Vec,
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

fn make_request(env: &Env, owner: &Address, clip_id: u32) -> MintRequest {
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, &format!("ipfs://QmBatchEvt{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient: Address::generate(env),
            basis_points: 500,
            asset_address: None,
        },
        creator_address: None,
        creator_display_name: None,
    }
}

fn find_batch_event(env: &Env, batch_id: u64) -> Option<BatchMintCompletedEvent> {
    for i in 0..env.events().all().events().len() {
        if let Ok((_, evt)) = env
            .events()
            .all()
            .events()
            .get(i)
            .map(|(t, d): (Vec<Val>, BatchMintCompletedEvent)| (t, d))
        {
            if evt.batch_id == batch_id {
                return Some(evt);
            }
        }
    }
    None
}

// ─── tests ────────────────────────────────────────────────────────────────────

/// A successful batch must emit a BatchMintCompletedEvent.
#[test]
fn test_batch_mint_completed_event_emitted_on_success() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [make_request(env, &owner, 1)]),
        };

        let resp = execute_batch_mint(env, &batch).expect("batch ok");

        assert!(
            find_batch_event(env, resp.batch_id).is_some(),
            "BatchMintCompletedEvent not found for batch_id {}",
            resp.batch_id
        );
    });
}

/// All four acceptance-criteria fields must match.
#[test]
fn test_batch_mint_completed_event_fields_correct() {
    with_contract(|env| {
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

        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    make_request(env, &owner, 10),
                    make_request(env, &owner, 11),
                    make_request(env, &owner, 12),
                ],
            ),
        };

        let resp = execute_batch_mint(env, &batch).expect("batch ok");
        let evt = find_batch_event(env, resp.batch_id).expect("event not found");

        assert_eq!(evt.batch_id, resp.batch_id, "batch_id mismatch");
        assert_eq!(evt.minted_count, 3, "minted_count should be 3");
        assert_eq!(
            evt.recipient, owner,
            "recipient should be first request owner"
        );
        assert_eq!(
            evt.timestamp, 1_720_000_000,
            "timestamp should match ledger"
        );
    });
}

/// minted_count must equal the number of requests in the batch.
#[test]
fn test_batch_mint_completed_event_minted_count_matches_batch_size() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    make_request(env, &owner, 20),
                    make_request(env, &owner, 21),
                    make_request(env, &owner, 22),
                    make_request(env, &owner, 23),
                    make_request(env, &owner, 24),
                ],
            ),
        };

        let resp = execute_batch_mint(env, &batch).expect("batch ok");
        let evt = find_batch_event(env, resp.batch_id).expect("event missing");

        assert_eq!(evt.minted_count, 5);
        assert_eq!(evt.minted_count, resp.success_count);
    });
}

/// Each successful batch call must emit its own event with a unique batch_id.
#[test]
fn test_batch_mint_completed_event_emitted_per_batch() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let batch1 = BatchMintRequest {
            requests: Vec::from_array(env, [make_request(env, &owner, 30)]),
        };
        let batch2 = BatchMintRequest {
            requests: Vec::from_array(env, [make_request(env, &owner, 31)]),
        };

        let resp1 = execute_batch_mint(env, &batch1).expect("batch1 ok");
        let resp2 = execute_batch_mint(env, &batch2).expect("batch2 ok");

        assert_ne!(resp1.batch_id, resp2.batch_id, "batch IDs must be unique");
        assert!(
            find_batch_event(env, resp1.batch_id).is_some(),
            "event for batch1 missing"
        );
        assert!(
            find_batch_event(env, resp2.batch_id).is_some(),
            "event for batch2 missing"
        );
    });
}

/// The timestamp must reflect the ledger time at batch completion.
#[test]
fn test_batch_mint_completed_event_timestamp_matches_ledger() {
    with_contract(|env| {
        env.ledger().set(LedgerInfo {
            timestamp: 5_555_555,
            protocol_version: 21,
            sequence_number: 3,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1,
            min_persistent_entry_ttl: 1,
            max_entry_ttl: 3_110_400,
        });

        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [make_request(env, &owner, 40)]),
        };

        let resp = execute_batch_mint(env, &batch).expect("batch ok");
        let evt = find_batch_event(env, resp.batch_id).expect("event missing");

        assert_eq!(evt.timestamp, 5_555_555);
    });
}
