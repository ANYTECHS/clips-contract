#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

use clips_nft::config::Config;
use clips_nft::mint_request::{BatchMintRequest, MintRequest};
use clips_nft::Royalty;
use clips_nft::types::Error;

#[test]
fn test_mint_request_fields() {
    let env = Env::default();
    let owner: Address = Address::generate(&env);
    let creator: Address = Address::generate(&env);
    let recipient: Address = Address::generate(&env);

    let royalty = Royalty {
        recipient: recipient.clone(),
        basis_points: 500,
        asset_address: None,
    };

    let req = MintRequest {
        clip_id: 42u32,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(&env, "ipfs://QmXyz"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: royalty.clone(),
    };

    assert_eq!(req.clip_id, 42u32);
    assert_eq!(req.owner, owner);
    assert_eq!(req.creator, creator);
    assert_eq!(req.metadata_uri, String::from_str(&env, "ipfs://QmXyz"));
}

#[test]
fn test_batch_mint_request_valid_size() {
    let env = Env::default();
    let owner: Address = Address::generate(&env);
    let creator: Address = Address::generate(&env);
    let recipient: Address = Address::generate(&env);
    let royalty = Royalty {
        recipient: recipient.clone(),
        basis_points: 500,
        asset_address: None,
    };

    let config = Config {
        owner: Address::generate(&env),
        version: 1,
        platform_fee_bps: 0,
        default_royalty_bps: 500,
        paused: false,
        max_batch_mint_size: 10,
        max_collection_size: 10_000,
    };

    let req1 = MintRequest {
        clip_id: 1,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(&env, "ipfs://Qm1"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: royalty.clone(),
    };
    let req2 = MintRequest {
        clip_id: 2,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(&env, "ipfs://Qm2"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: royalty.clone(),
    };

    let batch = BatchMintRequest {
        requests: Vec::from_array(&env, [req1, req2]),
    };

    assert!(batch.validate_batch_size(&config).is_ok());
}

#[test]
fn test_batch_mint_request_too_small() {
    let env = Env::default();

    let config = Config {
        owner: Address::generate(&env),
        version: 1,
        platform_fee_bps: 0,
        default_royalty_bps: 500,
        paused: false,
        max_batch_mint_size: 10,
        max_collection_size: 10_000,
    };

    let batch = BatchMintRequest {
        requests: Vec::new(&env),
    };

    assert_eq!(batch.validate_batch_size(&config), Err(Error::InvalidConfig));
}

#[test]
fn test_batch_mint_request_too_large() {
    let env = Env::default();
    let owner: Address = Address::generate(&env);
    let creator: Address = Address::generate(&env);
    let recipient: Address = Address::generate(&env);
    let royalty = Royalty {
        recipient: recipient.clone(),
        basis_points: 500,
        asset_address: None,
    };

    let config = Config {
        owner: Address::generate(&env),
        version: 1,
        platform_fee_bps: 0,
        default_royalty_bps: 500,
        paused: false,
        max_batch_mint_size: 2,
        max_collection_size: 10_000,
    };

    let req1 = MintRequest {
        clip_id: 1,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(&env, "ipfs://Qm1"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: royalty.clone(),
    };
    let req2 = MintRequest {
        clip_id: 2,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(&env, "ipfs://Qm2"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: royalty.clone(),
    };
    let req3 = MintRequest {
        clip_id: 3,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(&env, "ipfs://Qm3"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: royalty.clone(),
    };

    let batch = BatchMintRequest {
        requests: Vec::from_array(&env, [req1, req2, req3]),
    };

    assert_eq!(batch.validate_batch_size(&config), Err(Error::BatchLimitExceeded));
}
