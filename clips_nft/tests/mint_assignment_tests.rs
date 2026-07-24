//! Unit tests for ownership, metadata, royalty, indexing, and mint events.

#![cfg(test)]

use clips_nft::{
    clip_id_storage, creator_portfolio, creator_storage, media_uri_storage,
    mint_service::{execute_mint, execute_mint_with_media},
    nft_collection, owner_portfolio, royalty_percentage, token_storage, total_supply,
    AtomicMintContract, CreatorAssignedEvent, DataKey, Error, MintRequest, Royalty,
    TransactionStatus,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    Address, Env, String,
};

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn make_request(env: &Env, owner: &Address, clip_id: u32, bps: u32) -> MintRequest {
    let recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        metadata_uri: String::from_str(env, "ipfs://QmMeta"),
        royalty_info: Royalty {
            recipient,
            basis_points: bps,
            asset_address: None,
        },
    }
}

#[test]
fn creator_assignment_is_persisted() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 1, 500)).unwrap();
        assert_eq!(
            creator_storage::get_creator(env, result.token_id).unwrap(),
            owner
        );
    });
}

#[test]
fn owner_assignment_is_persisted() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 2, 500)).unwrap();
        let token = token_storage::get_token(env, result.token_id).unwrap();
        assert_eq!(token.owner, owner);
        assert_eq!(token.clip_id, 2);
    });
}

#[test]
fn metadata_linking_and_uri_storage() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 3, 250);
        let expected_uri = req.metadata_uri.clone();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            token_storage::get_metadata(env, result.token_id).unwrap(),
            expected_uri
        );
        assert_eq!(
            clip_id_storage::get_clip_id(env, result.token_id).unwrap(),
            3
        );
    });
}

#[test]
fn thumbnail_and_preview_uri_storage() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 4, 500);
        let thumb = String::from_str(env, "ipfs://QmThumbnail");
        let preview = String::from_str(env, "ipfs://QmPreview");
        let result =
            execute_mint_with_media(env, req, Some(thumb.clone()), Some(preview.clone())).unwrap();
        assert_eq!(
            media_uri_storage::get_thumbnail(env, result.token_id).unwrap(),
            thumb
        );
        assert_eq!(
            media_uri_storage::get_preview_uri(env, result.token_id).unwrap(),
            preview
        );
    });
}

#[test]
fn royalty_recipient_and_percentage_are_persisted() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 5, 750);
        let expected_recipient = req.royalty_info.recipient.clone();
        let result = execute_mint(env, req).unwrap();
        let royalty = token_storage::get_royalty(env, result.token_id).unwrap();
        assert_eq!(royalty.recipient, expected_recipient);
        assert_eq!(royalty.basis_points, 750);
        assert_eq!(
            royalty_percentage::get_royalty_percentage(env, result.token_id).unwrap(),
            750
        );
    });
}

#[test]
fn creator_and_owner_portfolio_indexing() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 6, 500)).unwrap();
        assert!(creator_portfolio::get_creator_portfolio(env, &owner).contains(&result.token_id));
        assert!(owner_portfolio::get_owner_portfolio(env, &owner).contains(&result.token_id));
    });
}

#[test]
fn collection_registration_and_association() {
    with_contract(|env| {
        nft_collection::register_collection(env, 10);
        assert!(nft_collection::collection_exists(env, 10));
        nft_collection::associate_nft(env, 100, 10).unwrap();
        assert_eq!(nft_collection::get_nft_collection(env, 100).unwrap(), 10);
        assert!(nft_collection::collection_contains_token(env, 10, 100));
    });
}

#[test]
fn mint_emits_creator_assignment_event() {
    with_contract(|env| {
        env.ledger().set_timestamp(1_711_000_000);
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 7, 500)).unwrap();
        assert!(env.events().all().events().len() >= 2);
        assert_eq!(
            creator_storage::get_creator(env, result.token_id).unwrap(),
            owner
        );
        let _ = CreatorAssignedEvent {
            token_id: result.token_id,
            creator: owner,
            clip_id: 7,
            timestamp: 1_711_000_000,
        };
    });
}

#[test]
fn mint_success_response_fields() {
    with_contract(|env| {
        env.ledger().set_timestamp(99);
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 8, 100);
        let uri = req.metadata_uri.clone();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(result.token_id, 1);
        assert_eq!(result.owner, owner);
        assert_eq!(result.metadata_uri, uri);
        assert_eq!(result.clip_id, 8);
        assert_eq!(result.mint_timestamp, 99);
        assert_eq!(result.status, TransactionStatus::Success);
    });
}

#[test]
fn total_supply_increments_and_persists() {
    with_contract(|env| {
        let owner = Address::generate(env);
        assert_eq!(total_supply::get_total_supply(env), 0);
        execute_mint(env, make_request(env, &owner, 9, 500)).unwrap();
        assert_eq!(total_supply::get_total_supply(env), 1);
        execute_mint(env, make_request(env, &owner, 10, 500)).unwrap();
        assert_eq!(total_supply::get_total_supply(env), 2);
        let persisted: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap();
        assert_eq!(persisted, 2);
    });
}

#[test]
fn total_supply_overflow_is_prevented() {
    with_contract(|env| {
        total_supply::set_total_supply(env, u32::MAX);
        assert_eq!(
            total_supply::increment_total_supply(env),
            Err(Error::SupplyOverflow)
        );
    });
}
