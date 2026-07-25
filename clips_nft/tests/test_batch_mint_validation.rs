//! Unit tests for batch mint pre-validation.
//!
//! Validates every mint request in a batch before processing begins:
//! - Owner
//! - Clip ID
//! - Metadata URI
//! - Royalties
//! - Duplicate clips
//! Aborts the entire batch if any validation check fails.

#![cfg(test)]

use clips_nft::{
    execute_batch_mint, total_supply, validate_batch_mint, AtomicMintContract, BatchMintRequest,
    DataKey, Error, MintRequest, Royalty, MAX_ROYALTY_BPS,
};
use soroban_sdk::{
    testutils::Address as _, Address, Env, String, Vec,
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
        metadata_uri: String::from_str(env, &format!("ipfs://QmBatchMeta{}", clip_id)),
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
// 1. Success Path Test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn valid_batch_passes_prevalidation_and_executes() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req1 = sample_request(env, &owner, 100, 500);
        let req2 = sample_request(env, &owner, 101, 750);
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert!(validate_batch_mint(env, &batch).is_ok());

        let response = execute_batch_mint(env, &batch).unwrap();
        assert_eq!(response.success_count, 2);
        assert_eq!(response.failure_count, 0);
        assert_eq!(total_supply::get_total_supply(env), 2);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Owner Pre-validation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_aborts_on_invalid_owner_contract_address() {
    with_contract(|env| {
        let contract_addr = env.current_contract_address();
        let valid_owner = Address::generate(env);

        let req1 = sample_request(env, &valid_owner, 200, 500);
        let mut req2 = sample_request(env, &contract_addr, 201, 500);
        req2.owner = contract_addr;

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::InvalidAddress));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::InvalidAddress));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

#[test]
fn batch_aborts_on_blacklisted_owner() {
    with_contract(|env| {
        let valid_owner = Address::generate(env);
        let blacklisted_owner = Address::generate(env);
        env.storage()
            .persistent()
            .set(&DataKey::Blacklisted(blacklisted_owner.clone()), &true);

        let req1 = sample_request(env, &valid_owner, 202, 500);
        let req2 = sample_request(env, &blacklisted_owner, 203, 500);

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::Unauthorized));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::Unauthorized));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Clip ID Pre-validation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_aborts_if_clip_id_already_minted_on_chain() {
    with_contract(|env| {
        let owner = Address::generate(env);

        // Pre-register clip_id 300 as minted
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(300), &true);

        let req1 = sample_request(env, &owner, 300, 500);
        let req2 = sample_request(env, &owner, 301, 500);

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::ClipAlreadyMinted));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::ClipAlreadyMinted));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Duplicate Clips Pre-validation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_aborts_if_duplicate_clip_id_in_batch() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let req1 = sample_request(env, &owner, 400, 500);
        let req2 = sample_request(env, &owner, 400, 750); // Duplicate clip_id 400!

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::ClipAlreadyMinted));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::ClipAlreadyMinted));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Metadata URI Pre-validation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_aborts_on_empty_metadata_uri() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let req1 = sample_request(env, &owner, 500, 500);
        let mut req2 = sample_request(env, &owner, 501, 500);
        req2.metadata_uri = String::from_str(env, "");

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::InvalidURI));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::InvalidURI));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

#[test]
fn batch_aborts_on_invalid_thumbnail_uri_scheme() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let req1 = sample_request(env, &owner, 502, 500);
        let mut req2 = sample_request(env, &owner, 503, 500);
        req2.thumbnail_uri = Some(String::from_str(env, "ftp://invalid.com/thumb.png"));

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::InvalidURI));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::InvalidURI));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Royalties Pre-validation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_aborts_on_invalid_royalty_basis_points() {
    with_contract(|env| {
        let owner = Address::generate(env);

        let req1 = sample_request(env, &owner, 600, 500);
        let req2 = sample_request(env, &owner, 601, MAX_ROYALTY_BPS + 1); // Exceeds limit!

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::InvalidBasisPoints));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::InvalidBasisPoints));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Configured Batch Limit Validation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn batch_aborts_when_request_count_exceeds_configured_limit() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let admin = Address::generate(env);

        let config = clips_nft::config::Config {
            owner: admin.clone(),
            version: 1,
            platform_fee_bps: 100,
            default_royalty_bps: 500,
            paused: false,
            max_batch_mint_size: 2,
            max_collection_size: 1_000,
        };
        env.storage().instance().set(&DataKey::Config, &config);

        let req1 = sample_request(env, &owner, 700, 500);
        let req2 = sample_request(env, &owner, 701, 500);
        let req3 = sample_request(env, &owner, 702, 500);

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2, req3]),
        };

        assert_eq!(validate_batch_mint(env, &batch), Err(Error::BatchLimitExceeded));
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::BatchLimitExceeded));
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

#[test]
fn batch_passes_when_request_count_is_within_configured_limit() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let admin = Address::generate(env);

        let config = clips_nft::config::Config {
            owner: admin.clone(),
            version: 1,
            platform_fee_bps: 100,
            default_royalty_bps: 500,
            paused: false,
            max_batch_mint_size: 3,
            max_collection_size: 1_000,
        };
        env.storage().instance().set(&DataKey::Config, &config);

        let req1 = sample_request(env, &owner, 710, 500);
        let req2 = sample_request(env, &owner, 711, 500);
        let req3 = sample_request(env, &owner, 712, 500);

        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2, req3]),
        };

        assert!(validate_batch_mint(env, &batch).is_ok());
        assert!(execute_batch_mint(env, &batch).is_ok());
        assert_eq!(total_supply::get_total_supply(env), 3);
    });
}
