//! Unit tests for atomic batch minting behavior.
//!
//! Acceptance Criteria:
//! - No partial mints
//! - Roll back storage updates
//! - Roll back events / counters
//! - Failure tests verifying zero state leaks when a batch fails.

#![cfg(test)]

use clips_nft::{
    clip_id_storage, creator_portfolio, execute_batch_mint, owner_portfolio, token_storage,
    total_supply, validate_batch_mint, wallet_token_index, AtomicMintContract, BatchMintRequest,
    BatchMintResponse, Error, MintRequest, Royalty,
};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String, Vec,
};

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn sample_request(env: &Env, owner: &Address, clip_id: u32, bps: u32) -> MintRequest {
    let recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, &format!("ipfs://QmAtomicBatch{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient,
            basis_points: bps,
            asset_address: None,
        },
        creator_address: None,
        creator_display_name: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Success Path Atomicity Test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn atomic_batch_minting_succeeds_for_all_valid_requests() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req1 = sample_request(env, &owner, 1, 500);
        let req2 = sample_request(env, &owner, 2, 500);
        let req3 = sample_request(env, &owner, 3, 500);

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2, req3]),
        };

        let response = execute_batch_mint(env, &batch).expect("batch mint should succeed");
        assert_eq!(response.success_count, 3);
        assert_eq!(response.failure_count, 0);
        assert_eq!(response.minted_token_ids.len(), 3);
        assert_eq!(total_supply::get_total_supply(env), 3);

        // Verify storage for all 3 tokens
        assert!(token_storage::token_exists(env, 1));
        assert!(token_storage::token_exists(env, 2));
        assert!(token_storage::token_exists(env, 3));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Pre-validation Failure Test (Zero State Changes)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn atomic_batch_aborts_on_validation_failure_with_zero_mints() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req1 = sample_request(env, &owner, 10, 500);
        let mut req2 = sample_request(env, &owner, 11, 500);
        req2.metadata_uri = String::from_str(env, ""); // Invalid empty URI!

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        let result = execute_batch_mint(env, &batch);
        assert_eq!(result, Err(Error::InvalidURI));

        // Verify zero tokens were created and storage is untouched
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
        assert!(!clip_id_storage::is_clip_mapped(env, 10));
        assert_eq!(wallet_token_index::get_wallet_tokens(env, &owner).len(), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Execution Failure Rollback Test (Reverts Prior Items in Batch)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn atomic_batch_rolls_back_prior_items_if_creation_fails() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);

        let mut req1 = sample_request(env, &owner, 100, 500);
        req1.creator_address = Some(creator.clone());

        let mut req2 = sample_request(env, &owner, 101, 500);
        req2.creator_address = Some(creator.clone());

        // Construct a batch where item 1 passes validation, but item 2 fails on-chain execution
        // by pre-registering clip_id 101 after validate_batch_mint checks, or triggering error.
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        // Simulate execution where pre-validation passes
        assert!(validate_batch_mint(env, &batch).is_ok());

        // Pre-map clip_id 101 directly in storage to trigger Error::ClipAlreadyMinted during execute_mint on item 2
        env.storage()
            .persistent()
            .set(&clips_nft::DataKey::ClipIdMinted(101), &true);

        let err = execute_batch_mint(env, &batch).expect_err("batch should fail on item 2");
        assert_eq!(err, Error::ClipAlreadyMinted);

        // Verify NO partial mints occurred for item 1
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
        assert!(!clip_id_storage::is_clip_mapped(env, 100));

        // Verify owner portfolio and wallet index rolled back for item 1
        assert!(!owner_portfolio::owner_contains_token(env, &owner, 1));
        assert!(!wallet_token_index::get_wallet_tokens(env, &owner).contains(&1));

        // Verify creator portfolio rolled back for item 1
        assert!(!creator_portfolio::creator_contains_token(env, &creator, 1));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Duplicate Within Batch Rejection (No Partial Mints)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn atomic_batch_rejects_duplicate_clip_ids_within_batch() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req1 = sample_request(env, &owner, 50, 500);
        let req2 = sample_request(env, &owner, 51, 500);
        let req3 = sample_request(env, &owner, 50, 500); // Duplicate clip_id 50!

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2, req3]),
        };

        let err = execute_batch_mint(env, &batch).expect_err("duplicate clip ID in batch should fail");
        assert_eq!(err, Error::ClipAlreadyMinted);

        // Verify zero partial mints occurred
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
        assert!(!token_storage::token_exists(env, 2));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Reusable BatchMintResponse Object Tests (Acceptance Criteria)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_mint_response_contains_all_required_fields() {
    with_contract(|env| {
        env.ledger().set_timestamp(1_720_000_123);

        let owner = Address::generate(env);
        let req1 = sample_request(env, &owner, 200, 500);
        let req2 = sample_request(env, &owner, 201, 500);
        let req3 = sample_request(env, &owner, 202, 500);
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2, req3]),
        };

        let response: BatchMintResponse =
            execute_batch_mint(env, &batch).expect("batch mint should succeed");

        // 1. Batch ID
        assert_eq!(
            response.batch_id, 0,
            "first batch invocation must receive batch_id=0 (counter starts at 0)"
        );

        // 2. Minted token IDs
        assert_eq!(
            response.minted_token_ids.len(),
            3,
            "minted_token_ids must contain one entry per NFT"
        );
        assert_eq!(
            response.minted_token_ids.get(0).unwrap(),
            1,
            "first token in batch must be id=1"
        );
        assert_eq!(
            response.minted_token_ids.get(1).unwrap(),
            2,
            "second token in batch must be id=2"
        );
        assert_eq!(
            response.minted_token_ids.get(2).unwrap(),
            3,
            "third token in batch must be id=3"
        );

        // 3. Success count
        assert_eq!(
            response.success_count, 3,
            "success_count must equal number of NFTs created"
        );

        // 4. Failure count (atomic all-or-nothing → 0 on success)
        assert_eq!(
            response.failure_count, 0,
            "failure_count must be 0 when batch completes atomically"
        );

        // 5. Processing timestamp (matches ledger timestamp at end of call)
        assert_eq!(
            response.processed_at, 1_720_000_123,
            "processed_at must reflect the ledger timestamp"
        );
    });
}

#[test]
fn batch_ids_are_strictly_monotonic_across_invocations() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let batch = |clip_start: u32| {
            let reqs = Vec::from_array(
                env,
                [
                    sample_request(env, &owner, clip_start, 500),
                    sample_request(env, &owner, clip_start + 1, 500),
                ],
            );
            BatchMintRequest { requests: reqs }
        };

        let r1 = execute_batch_mint(env, &batch(0)).unwrap();
        let r2 = execute_batch_mint(env, &batch(10)).unwrap();
        let r3 = execute_batch_mint(env, &batch(20)).unwrap();

        assert_eq!(r1.batch_id, 0);
        assert_eq!(r2.batch_id, 1);
        assert_eq!(r3.batch_id, 2);
        assert_eq!(r1.success_count, 2);
        assert_eq!(r2.success_count, 2);
        assert_eq!(r3.success_count, 2);
        assert_eq!(r1.minted_token_ids.get(1).unwrap(), 2);
        assert_eq!(r3.minted_token_ids.get(0).unwrap(), 5);
    });
}

#[test]
fn validation_failure_still_consumes_a_batch_id() {
    with_contract(|env| {
        let owner = Address::generate(env);

        // First call: 2 good requests → batch_id 0, succeeds
        let good_batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    sample_request(env, &owner, 500, 500),
                    sample_request(env, &owner, 501, 500),
                ],
            ),
        };
        let ok = execute_batch_mint(env, &good_batch).unwrap();
        assert_eq!(ok.batch_id, 0);

        // Second call: bad request (empty metadata uri) → fails, but counter bumped
        let mut bad = sample_request(env, &owner, 502, 500);
        bad.metadata_uri = String::from_str(env, "");
        let bad_batch = BatchMintRequest {
            requests: Vec::from_array(env, [bad]),
        };
        let err = execute_batch_mint(env, &bad_batch).unwrap_err();
        assert_eq!(err, Error::InvalidURI);

        // Third call: another good batch → batch_id should be 2 (one id was consumed by the fail)
        let good2 = BatchMintRequest {
            requests: Vec::from_array(env, [sample_request(env, &owner, 503, 500)]),
        };
        let ok2 = execute_batch_mint(env, &good2).unwrap();
        assert_eq!(
            ok2.batch_id, 2,
            "a failed validation must still consume a batch id so ids are never reused"
        );
        assert_eq!(ok2.success_count, 1);
    });
}

#[test]
fn batch_mint_response_success_count_equals_token_ids_len() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    sample_request(env, &owner, 600, 500),
                    sample_request(env, &owner, 601, 500),
                    sample_request(env, &owner, 602, 500),
                    sample_request(env, &owner, 603, 500),
                ],
            ),
        };
        let r = execute_batch_mint(env, &batch).unwrap();
        assert_eq!(
            r.success_count,
            r.minted_token_ids.len() as u32,
            "success_count and minted_token_ids.len() must be consistent"
        );
        assert_eq!(r.success_count, 4);
        assert_eq!(r.failure_count, 0);
    });
}
