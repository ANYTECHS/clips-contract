#![cfg(test)]
use clips_nft::{BatchMintRequest, Config, Error, MintRequest, Royalty, RoyaltyRecipient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn mint_request(env: &Env) -> MintRequest {
    let owner = Address::generate(env);
    MintRequest {
        clip_id: 1,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, "ipfs://test"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipients: soroban_sdk::vec![
                env,
                RoyaltyRecipient {
                    recipient: owner,
                    basis_points: 500,
                },
            ],
            asset_address: None,
        },
        creator_address: None,
        creator_display_name: None,
    }
}

fn batch_with_size(env: &Env, size: u32) -> BatchMintRequest {
    let mut requests = soroban_sdk::Vec::new(env);
    for _ in 0..size {
        requests.push_back(mint_request(env));
    }
    BatchMintRequest { requests }
}

fn config(env: &Env, max_batch_mint_size: u32) -> Config {
    Config {
        owner: Address::generate(env),
        version: 1,
        platform_fee_bps: 0,
        default_royalty_bps: 500,
        paused: false,
        max_batch_mint_size,
        max_batch_transfer_size: 1,
        max_collection_size: 100,
    }
}

#[test]
fn rejects_zero_mint_quantity() {
    let env = Env::default();
    let batch = batch_with_size(&env, 0);

    assert_eq!(
        batch.validate_batch_size(&config(&env, 1)),
        Err(Error::InvalidConfig)
    );
}

#[test]
fn accepts_minimum_mint_quantity() {
    let env = Env::default();
    let batch = batch_with_size(&env, 1);

    assert!(batch.validate_batch_size(&config(&env, 1)).is_ok());
}

#[test]
fn accepts_configured_maximum_mint_quantity() {
    let env = Env::default();
    let batch = batch_with_size(&env, 3);

    assert!(batch.validate_batch_size(&config(&env, 3)).is_ok());
}

#[test]
fn rejects_mint_quantity_above_configured_maximum() {
    let env = Env::default();
    let batch = batch_with_size(&env, 4);

    assert_eq!(
        batch.validate_batch_size(&config(&env, 3)),
        Err(Error::BatchLimitExceeded)
    );
}
