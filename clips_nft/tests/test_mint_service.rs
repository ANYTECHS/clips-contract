//! Integration tests for the mint service module.
//!
//! Covers the acceptance criteria for issue #651:
//! - Receive validated mint request
//! - Generate token ID
//! - Create NFT (writes all storage entries)
//! - Return mint result

#![cfg(test)]

use clips_nft::{
    clip_id_storage, mint_service::execute_mint, mint_service::MintResult, token_storage,
    wallet_token_index, DataKey, Error, MintRequest, Royalty, TokenId,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn make_request(env: &Env, clip_id: u32) -> MintRequest {
    let owner = Address::generate(env);
    let royalty_recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner,
        metadata_uri: String::from_str(env, &format!("ipfs://QmClip{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient: royalty_recipient,
            basis_points: 500,
            asset_address: None,
        },
    }
}

fn make_request_with_owner(env: &Env, owner: &Address, clip_id: u32) -> MintRequest {
    let royalty_recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, &format!("ipfs://QmOwnerClip{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient: royalty_recipient,
            basis_points: 250,
            asset_address: None,
        },
    }
}

// ─── Token ID generation ──────────────────────────────────────────────────────

/// The very first mint must assign token_id = 1.
#[test]
fn test_first_mint_token_id_is_one() {
    let env = Env::default();
    let result = execute_mint(&env, make_request(&env, 1)).expect("first mint ok");
    assert_eq!(result.token_id, 1, "first token id must be 1");
}

/// Sequential mints produce strictly increasing token IDs.
#[test]
fn test_sequential_mints_produce_incrementing_ids() {
    let env = Env::default();
    let ids: Vec<TokenId> = (1u32..=5)
        .map(|i| {
            execute_mint(&env, make_request(&env, i))
                .expect("mint ok")
                .token_id
        })
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
}

// ─── MintResult contents ─────────────────────────────────────────────────────

/// MintResult fields must exactly mirror the input request.
#[test]
fn test_mint_result_reflects_request() {
    let env = Env::default();
    let req = make_request(&env, 100);

    let expected_clip_id = req.clip_id;
    let expected_owner = req.owner.clone();
    let expected_uri = req.metadata_uri.clone();

    let result: MintResult = execute_mint(&env, req).expect("mint ok");

    assert_eq!(result.clip_id, expected_clip_id);
    assert_eq!(result.owner, expected_owner);
    assert_eq!(result.metadata_uri, expected_uri);
}

// ─── NFT creation (storage writes) ───────────────────────────────────────────

/// Token data (owner + clip_id) must be persisted after mint.
#[test]
fn test_mint_persists_token_data() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let req = make_request_with_owner(&env, &owner, 200);
    let clip_id = req.clip_id;

    let result = execute_mint(&env, req).expect("mint ok");
    let token = token_storage::get_token(&env, result.token_id).expect("token should exist");

    assert_eq!(token.owner, owner, "owner must match");
    assert_eq!(token.clip_id, clip_id, "clip_id must match");
}

/// Metadata URI must be persisted after mint.
#[test]
fn test_mint_persists_metadata_uri() {
    let env = Env::default();
    let req = make_request(&env, 201);
    let expected_uri = req.metadata_uri.clone();

    let result = execute_mint(&env, req).expect("mint ok");
    let uri = token_storage::get_metadata(&env, result.token_id).expect("metadata should exist");

    assert_eq!(uri, expected_uri, "URI must match request");
}

/// Royalty config must be persisted after mint.
#[test]
fn test_mint_persists_royalty() {
    let env = Env::default();
    let req = make_request(&env, 202);
    let expected_bps = req.royalty_info.basis_points;

    let result = execute_mint(&env, req).expect("mint ok");
    let royalty = token_storage::get_royalty(&env, result.token_id).expect("royalty should exist");

    assert_eq!(royalty.basis_points, expected_bps);
}

/// The clip_id → token_id forward mapping must be recorded.
#[test]
fn test_mint_records_clip_id_forward_mapping() {
    let env = Env::default();
    let req = make_request(&env, 300);
    let clip_id = req.clip_id;

    execute_mint(&env, req).expect("mint ok");

    assert!(
        env.storage()
            .persistent()
            .has(&DataKey::ClipIdMinted(clip_id)),
        "ClipIdMinted sentinel must be set"
    );
}

/// The token_id → clip_id reverse mapping must be recorded.
#[test]
fn test_mint_records_clip_id_reverse_mapping() {
    let env = Env::default();
    let req = make_request(&env, 301);
    let clip_id = req.clip_id;

    let result = execute_mint(&env, req).expect("mint ok");
    let stored =
        clip_id_storage::get_clip_id(&env, result.token_id).expect("reverse mapping should exist");

    assert_eq!(stored, clip_id);
}

/// The token must be present in the owner's wallet index after mint.
#[test]
fn test_mint_adds_token_to_wallet_index() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let req = make_request_with_owner(&env, &owner, 400);

    let result = execute_mint(&env, req).expect("mint ok");
    let tokens = wallet_token_index::get_wallet_tokens(&env, &owner);

    assert!(
        tokens.contains(&result.token_id),
        "token should appear in owner wallet index"
    );
}

/// Multiple tokens owned by the same wallet must all appear in the index.
#[test]
fn test_wallet_index_accumulates_multiple_tokens() {
    let env = Env::default();
    let owner = Address::generate(&env);

    let r1 = execute_mint(&env, make_request_with_owner(&env, &owner, 500)).unwrap();
    let r2 = execute_mint(&env, make_request_with_owner(&env, &owner, 501)).unwrap();
    let r3 = execute_mint(&env, make_request_with_owner(&env, &owner, 502)).unwrap();

    let tokens = wallet_token_index::get_wallet_tokens(&env, &owner);
    assert!(tokens.contains(&r1.token_id));
    assert!(tokens.contains(&r2.token_id));
    assert!(tokens.contains(&r3.token_id));
}

// ─── Total supply ─────────────────────────────────────────────────────────────

/// TotalSupply in instance storage must increment with each mint.
#[test]
fn test_total_supply_increments_on_each_mint() {
    let env = Env::default();

    let supply = |env: &Env| -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    };

    assert_eq!(supply(&env), 0, "initial supply must be 0");
    execute_mint(&env, make_request(&env, 600)).unwrap();
    assert_eq!(supply(&env), 1);
    execute_mint(&env, make_request(&env, 601)).unwrap();
    assert_eq!(supply(&env), 2);
}

// ─── Duplicate-mint prevention ────────────────────────────────────────────────

/// Re-minting the same clip_id must return ClipAlreadyMinted.
#[test]
fn test_duplicate_clip_id_returns_already_minted() {
    let env = Env::default();

    execute_mint(&env, make_request(&env, 700)).expect("first mint ok");
    let err = execute_mint(&env, make_request(&env, 700)).expect_err("duplicate mint must fail");

    assert_eq!(err, Error::ClipAlreadyMinted);
}

/// A failed duplicate mint must not change the token counter.
#[test]
fn test_duplicate_mint_does_not_increment_supply() {
    let env = Env::default();

    execute_mint(&env, make_request(&env, 800)).expect("first mint ok");

    let _ = execute_mint(&env, make_request(&env, 800));

    let supply: u32 = env
        .storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0);
    assert_eq!(supply, 1, "supply must stay at 1 after failed duplicate");
}

// ─── Invalid input ────────────────────────────────────────────────────────────

/// An empty metadata URI must return InvalidURI.
#[test]
fn test_empty_metadata_uri_returns_invalid_uri() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);

    let req = MintRequest {
        clip_id: 900,
        owner: owner.clone(),
        creator: owner,
        metadata_uri: String::from_str(&env, ""),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient,
            basis_points: 0,
            asset_address: None,
        },
    };

    let err = execute_mint(&env, req).expect_err("empty URI must fail");
    assert_eq!(err, Error::InvalidURI);
}
