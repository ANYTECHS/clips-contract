//! Comprehensive unit test suite covering ownership assignment, metadata association,
//! creator indexing, royalty persistence, collection registration, and event emission.

#![cfg(test)]

use clips_nft::{
    clip_id_storage, creator_event, creator_portfolio, creator_storage, media_uri_storage,
    mint_event, mint_metadata_link, mint_metadata_uri,
    mint_service::{execute_mint, execute_mint_with_media},
    nft_collection, owner_portfolio, preview_video_uri, royalty_percentage, royalty_recipient,
    thumbnail_uri, token_owner_storage, token_storage, total_supply, wallet_token_index,
    AtomicMintContract, DataKey, Error, MintRequest, Royalty, TransactionStatus, MAX_ROYALTY_BPS,
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
        creator: owner.clone(),
        metadata_uri: String::from_str(env, "ipfs://QmMeta"),
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
// 1. Creator Assignment Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn creator_assignment_defaults_to_owner() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 101, 500)).unwrap();

        let stored_creator = creator_storage::get_creator(env, result.token_id).unwrap();
        assert_eq!(stored_creator, owner);

        let metadata = creator_storage::get_creator_metadata(env, result.token_id).unwrap();
        assert_eq!(metadata.creator_address, owner);
        assert_eq!(metadata.display_name, None);
        assert!(!metadata.verified);
    });
}

#[test]
fn creator_assignment_with_explicit_creator_and_display_name() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let display_name = Some(String::from_str(env, "ArtistName"));

        let mut req = make_request(env, &owner, 102, 500);
        req.creator_address = Some(creator.clone());
        req.creator_display_name = display_name.clone();

        let result = execute_mint(env, req).unwrap();

        // get_creator derives the address from the stored CreatorMetadata; it
        // must return the explicit creator, not the owner.
        let stored_creator = creator_storage::get_creator(env, result.token_id).unwrap();
        assert_eq!(stored_creator, creator);

        let metadata = creator_storage::get_creator_metadata(env, result.token_id).unwrap();
        assert_eq!(metadata.creator_address, creator);
        assert_eq!(metadata.display_name, display_name);
        assert!(!metadata.verified);

        let stored_name = creator_storage::get_creator_display_name(env, result.token_id).unwrap();
        assert_eq!(stored_name, display_name);
    });
}

#[test]
fn creator_assignment_scoped_per_token() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator_a = Address::generate(env);
        let creator_b = Address::generate(env);

        let mut req_a = make_request(env, &owner, 103, 500);
        req_a.creator_address = Some(creator_a.clone());

        let mut req_b = make_request(env, &owner, 104, 500);
        req_b.creator_address = Some(creator_b.clone());

        let res_a = execute_mint(env, req_a).unwrap();
        let res_b = execute_mint(env, req_b).unwrap();

        let meta_a = creator_storage::get_creator_metadata(env, res_a.token_id).unwrap();
        let meta_b = creator_storage::get_creator_metadata(env, res_b.token_id).unwrap();

        assert_eq!(meta_a.creator_address, creator_a);
        assert_eq!(meta_b.creator_address, creator_b);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Owner Assignment Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn owner_assignment_is_persisted_on_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 201, 500)).unwrap();

        let token = token_storage::get_token(env, result.token_id).unwrap();
        assert_eq!(token.owner, owner);
        assert_eq!(token.clip_id, 201);

        let owner_from_storage = token_owner_storage::get_owner(env, result.token_id).unwrap();
        assert_eq!(owner_from_storage, owner);
    });
}

#[test]
fn owner_assignment_rejects_contract_address() {
    with_contract(|env| {
        let contract_addr = env.current_contract_address();
        assert_eq!(
            token_owner_storage::validate_owner(env, &contract_addr),
            Err(Error::InvalidAddress)
        );
    });
}

#[test]
fn owner_assignment_isolation_across_tokens() {
    with_contract(|env| {
        let alice = Address::generate(env);
        let bob = Address::generate(env);

        let res_a = execute_mint(env, make_request(env, &alice, 202, 500)).unwrap();
        let res_b = execute_mint(env, make_request(env, &bob, 203, 500)).unwrap();

        assert_eq!(
            token_storage::get_token(env, res_a.token_id).unwrap().owner,
            alice
        );
        assert_eq!(
            token_storage::get_token(env, res_b.token_id).unwrap().owner,
            bob
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Metadata Linking Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn metadata_linking_bidirectional_and_existence_index() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 301, 500)).unwrap();

        assert_eq!(
            clip_id_storage::get_clip_id(env, result.token_id).unwrap(),
            301
        );
        assert!(clip_id_storage::is_clip_mapped(env, 301));
        // ClipIdMinted is the canonical dedup guard; no separate ClipMinted marker is written.
    });
}

#[test]
fn metadata_linking_to_nft_record() {
    with_contract(|env| {
        let uri = String::from_str(env, "ipfs://QmRegisteredMeta");
        mint_metadata_link::register_metadata_record(env, &uri).unwrap();
        assert!(mint_metadata_link::metadata_record_exists(env, &uri));

        let token_id = 10u32;
        mint_metadata_link::link_metadata_to_nft(env, token_id, &uri).unwrap();

        assert_eq!(
            mint_metadata_link::get_linked_metadata(env, token_id).unwrap(),
            uri
        );
        assert!(mint_metadata_link::token_has_metadata_link(env, token_id));
    });
}

#[test]
fn metadata_linking_duplicate_clip_id_rejected() {
    with_contract(|env| {
        let owner = Address::generate(env);
        execute_mint(env, make_request(env, &owner, 302, 500)).unwrap();

        let err = execute_mint(env, make_request(env, &owner, 302, 500)).unwrap_err();
        assert_eq!(err, Error::ClipAlreadyMinted);
    });
}

#[test]
fn metadata_linking_unregistered_record_rejected() {
    with_contract(|env| {
        let uri = String::from_str(env, "ipfs://QmUnregistered");
        assert_eq!(
            mint_metadata_link::link_metadata_to_nft(env, 1, &uri),
            Err(Error::MetadataNotFound)
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Metadata URI Storage Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn metadata_uri_storage_ipfs_and_https() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let mut req_ipfs = make_request(env, &owner, 401, 500);
        req_ipfs.metadata_uri = String::from_str(env, "ipfs://QmMeta401");

        let res_ipfs = execute_mint(env, req_ipfs).unwrap();
        assert_eq!(
            token_storage::get_metadata(env, res_ipfs.token_id).unwrap(),
            String::from_str(env, "ipfs://QmMeta401")
        );

        let mut req_https = make_request(env, &owner, 402, 500);
        req_https.metadata_uri = String::from_str(env, "https://example.com/meta402.json");

        let res_https = execute_mint(env, req_https).unwrap();
        assert_eq!(
            token_storage::get_metadata(env, res_https.token_id).unwrap(),
            String::from_str(env, "https://example.com/meta402.json")
        );
    });
}

#[test]
fn metadata_uri_storage_empty_uri_rejected() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let mut req = make_request(env, &owner, 403, 500);
        req.metadata_uri = String::from_str(env, "");

        assert_eq!(execute_mint(env, req).unwrap_err(), Error::InvalidURI);
    });
}

#[test]
fn metadata_uri_storage_module_functions() {
    with_contract(|env| {
        let uri = String::from_str(env, "ipfs://QmDirectStorage");
        mint_metadata_uri::set_metadata_uri(env, 1, &uri).unwrap();
        assert_eq!(mint_metadata_uri::get_metadata_uri(env, 1).unwrap(), uri);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Thumbnail Storage Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn thumbnail_storage_and_retrieval() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 501, 500);
        let thumb = String::from_str(env, "ipfs://QmThumb501");

        let result = execute_mint_with_media(env, req, Some(thumb.clone()), None).unwrap();

        assert_eq!(
            media_uri_storage::get_thumbnail(env, result.token_id).unwrap(),
            thumb
        );
        assert_eq!(
            thumbnail_uri::get_thumbnail_uri(env, result.token_id),
            Some(thumb)
        );
    });
}

#[test]
fn thumbnail_storage_none_when_omitted() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 502, 500)).unwrap();

        assert_eq!(media_uri_storage::get_thumbnail(env, result.token_id), None);
        assert_eq!(thumbnail_uri::get_thumbnail_uri(env, result.token_id), None);
    });
}

#[test]
fn thumbnail_storage_invalid_scheme_rejected() {
    with_contract(|env| {
        let bad_thumb = String::from_str(env, "ftp://bad.example.com/thumb.png");
        assert_eq!(
            thumbnail_uri::set_thumbnail_uri(env, 1, &bad_thumb).unwrap_err(),
            Error::InvalidURI
        );
    });
}

#[test]
fn thumbnail_storage_overwritten() {
    with_contract(|env| {
        let thumb_v1 = String::from_str(env, "ipfs://QmThumbV1");
        let thumb_v2 = String::from_str(env, "ipfs://QmThumbV2");

        thumbnail_uri::set_thumbnail_uri(env, 503, &thumb_v1).unwrap();
        thumbnail_uri::set_thumbnail_uri(env, 503, &thumb_v2).unwrap();

        assert_eq!(thumbnail_uri::get_thumbnail_uri(env, 503), Some(thumb_v2));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Preview URI Storage Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn preview_uri_storage_and_retrieval() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 601, 500);
        let preview = String::from_str(env, "ipfs://QmPreview601");

        let result = execute_mint_with_media(env, req, None, Some(preview.clone())).unwrap();

        assert_eq!(
            media_uri_storage::get_preview_uri(env, result.token_id).unwrap(),
            preview
        );
        assert_eq!(
            preview_video_uri::get_preview_video_uri(env, result.token_id),
            Some(preview)
        );
    });
}

#[test]
fn preview_uri_storage_none_when_omitted() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 602, 500)).unwrap();

        assert_eq!(
            media_uri_storage::get_preview_uri(env, result.token_id),
            None
        );
        assert_eq!(
            preview_video_uri::get_preview_video_uri(env, result.token_id),
            None
        );
    });
}

#[test]
fn preview_uri_storage_invalid_scheme_rejected() {
    with_contract(|env| {
        let bad_preview = String::from_str(env, "http://unsecure.example.com/preview.mp4");
        assert_eq!(
            preview_video_uri::set_preview_video_uri(env, 1, &bad_preview).unwrap_err(),
            Error::InvalidURI
        );
    });
}

#[test]
fn preview_uri_storage_scoped_per_token() {
    with_contract(|env| {
        let prev_a = String::from_str(env, "ipfs://QmPreviewA");
        let prev_b = String::from_str(env, "ipfs://QmPreviewB");

        preview_video_uri::set_preview_video_uri(env, 603, &prev_a).unwrap();
        preview_video_uri::set_preview_video_uri(env, 604, &prev_b).unwrap();

        assert_eq!(
            preview_video_uri::get_preview_video_uri(env, 603),
            Some(prev_a)
        );
        assert_eq!(
            preview_video_uri::get_preview_video_uri(env, 604),
            Some(prev_b)
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Royalty Recipient Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn royalty_recipient_persisted_on_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = make_request(env, &owner, 701, 750);
        let expected_recipient = req.royalty_info.recipient.clone();

        let result = execute_mint(env, req).unwrap();

        let royalty = token_storage::get_royalty(env, result.token_id).unwrap();
        assert_eq!(royalty.recipient, expected_recipient);
        assert_eq!(
            royalty_recipient::get_royalty_recipient(env, result.token_id),
            expected_recipient
        );
    });
}

#[test]
fn royalty_recipient_admin_update() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let new_recipient = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 702, 500)).unwrap();

        royalty_recipient::update_royalty_recipient(env, result.token_id, &new_recipient);
        assert_eq!(
            royalty_recipient::get_royalty_recipient(env, result.token_id),
            new_recipient
        );
    });
}

#[test]
fn royalty_recipient_scoped_per_token() {
    with_contract(|env| {
        let recipient_a = Address::generate(env);
        let recipient_b = Address::generate(env);

        royalty_recipient::set_royalty_recipient(env, 1, &recipient_a);
        royalty_recipient::set_royalty_recipient(env, 2, &recipient_b);

        assert_eq!(
            royalty_recipient::get_royalty_recipient(env, 1),
            recipient_a
        );
        assert_eq!(
            royalty_recipient::get_royalty_recipient(env, 2),
            recipient_b
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Royalty Percentage Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn royalty_percentage_persisted_and_retrieved() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 801, 750)).unwrap();

        assert_eq!(
            royalty_percentage::get_royalty_percentage(env, result.token_id).unwrap(),
            750
        );
    });
}

#[test]
fn royalty_percentage_boundary_values() {
    with_contract(|env| {
        royalty_percentage::set_royalty_percentage(env, 1, 0).unwrap();
        assert_eq!(
            royalty_percentage::get_royalty_percentage(env, 1).unwrap(),
            0
        );

        royalty_percentage::set_royalty_percentage(env, 2, MAX_ROYALTY_BPS).unwrap();
        assert_eq!(
            royalty_percentage::get_royalty_percentage(env, 2).unwrap(),
            MAX_ROYALTY_BPS
        );
    });
}

#[test]
fn royalty_percentage_exceeding_max_rejected() {
    with_contract(|env| {
        assert_eq!(
            royalty_percentage::set_royalty_percentage(env, 1, MAX_ROYALTY_BPS + 1),
            Err(Error::InvalidBasisPoints)
        );
    });
}

#[test]
fn royalty_percentage_unrecorded_token_returns_not_found() {
    with_contract(|env| {
        assert_eq!(
            royalty_percentage::get_royalty_percentage(env, 999),
            Err(Error::TokenNotFound)
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Creator Portfolio Indexing Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn creator_portfolio_indexing_on_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let mut req = make_request(env, &owner, 901, 500);
        req.creator_address = Some(creator.clone());

        let result = execute_mint(env, req).unwrap();

        let portfolio = creator_portfolio::get_creator_portfolio(env, &creator);
        assert_eq!(portfolio.len(), 1);
        assert_eq!(portfolio.get(0).unwrap(), result.token_id);
        assert!(creator_portfolio::creator_contains_token(
            env,
            &creator,
            result.token_id
        ));
    });
}

#[test]
fn creator_portfolio_preserves_insertion_order() {
    with_contract(|env| {
        let creator = Address::generate(env);
        creator_portfolio::add_token_to_creator(env, &creator, 10).unwrap();
        creator_portfolio::add_token_to_creator(env, &creator, 20).unwrap();
        creator_portfolio::add_token_to_creator(env, &creator, 30).unwrap();

        let portfolio = creator_portfolio::get_creator_portfolio(env, &creator);
        assert_eq!(portfolio.len(), 3);
        assert_eq!(portfolio.get(0).unwrap(), 10);
        assert_eq!(portfolio.get(1).unwrap(), 20);
        assert_eq!(portfolio.get(2).unwrap(), 30);
    });
}

#[test]
fn creator_portfolio_rejects_duplicate() {
    with_contract(|env| {
        let creator = Address::generate(env);
        creator_portfolio::add_token_to_creator(env, &creator, 5).unwrap();
        assert_eq!(
            creator_portfolio::add_token_to_creator(env, &creator, 5),
            Err(Error::DuplicateRecord)
        );
    });
}

#[test]
fn creator_portfolio_isolation_and_empty_check() {
    with_contract(|env| {
        let alice = Address::generate(env);
        let bob = Address::generate(env);
        creator_portfolio::add_token_to_creator(env, &alice, 1).unwrap();

        assert_eq!(
            creator_portfolio::get_creator_portfolio(env, &alice).len(),
            1
        );
        assert_eq!(creator_portfolio::get_creator_portfolio(env, &bob).len(), 0);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Owner Portfolio Indexing Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn owner_portfolio_indexing_on_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request(env, &owner, 1001, 500)).unwrap();

        let portfolio = owner_portfolio::get_owner_portfolio(env, &owner);
        assert_eq!(portfolio.len(), 1);
        assert_eq!(portfolio.get(0).unwrap(), result.token_id);
        assert!(owner_portfolio::owner_contains_token(
            env,
            &owner,
            result.token_id
        ));

        let wallet_tokens = wallet_token_index::get_wallet_tokens(env, &owner);
        assert!(wallet_tokens.contains(&result.token_id));
    });
}

#[test]
fn owner_portfolio_preserves_insertion_order() {
    with_contract(|env| {
        let owner = Address::generate(env);
        owner_portfolio::add_token_to_owner(env, &owner, 300).unwrap();
        owner_portfolio::add_token_to_owner(env, &owner, 100).unwrap();
        owner_portfolio::add_token_to_owner(env, &owner, 200).unwrap();

        let portfolio = owner_portfolio::get_owner_portfolio(env, &owner);
        assert_eq!(portfolio.len(), 3);
        assert_eq!(portfolio.get(0).unwrap(), 300);
        assert_eq!(portfolio.get(1).unwrap(), 100);
        assert_eq!(portfolio.get(2).unwrap(), 200);
    });
}

#[test]
fn owner_portfolio_rejects_duplicate() {
    with_contract(|env| {
        let owner = Address::generate(env);
        owner_portfolio::add_token_to_owner(env, &owner, 7).unwrap();
        assert_eq!(
            owner_portfolio::add_token_to_owner(env, &owner, 7),
            Err(Error::DuplicateRecord)
        );
    });
}

#[test]
fn wallet_token_index_removal() {
    with_contract(|env| {
        let owner = Address::generate(env);
        wallet_token_index::add_token_to_wallet(env, &owner, 50).unwrap();
        assert!(wallet_token_index::get_wallet_tokens(env, &owner).contains(&50));

        wallet_token_index::remove_token_from_wallet(env, &owner, 50);
        assert!(!wallet_token_index::get_wallet_tokens(env, &owner).contains(&50));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. Collection Registration Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn collection_registration_and_association() {
    with_contract(|env| {
        assert!(!nft_collection::collection_exists(env, 110));
        nft_collection::register_collection(env, 110);
        assert!(nft_collection::collection_exists(env, 110));

        nft_collection::associate_nft(env, 100, 110).unwrap();

        assert_eq!(nft_collection::get_nft_collection(env, 100).unwrap(), 110);
        assert!(nft_collection::collection_contains_token(env, 110, 100));

        let members = nft_collection::get_collection_members(env, 110);
        assert_eq!(members.len(), 1);
        assert_eq!(members.get(0).unwrap(), 100);
    });
}

#[test]
fn collection_registration_rejects_unregistered_collection() {
    with_contract(|env| {
        assert_eq!(
            nft_collection::associate_nft(env, 100, 999),
            Err(Error::CollectionNotFound)
        );
    });
}

#[test]
fn collection_registration_prevents_duplicate_membership() {
    with_contract(|env| {
        nft_collection::register_collection(env, 111);
        nft_collection::associate_nft(env, 100, 111).unwrap();
        assert_eq!(
            nft_collection::associate_nft(env, 100, 111),
            Err(Error::DuplicateRecord)
        );
    });
}

#[test]
fn collection_registration_unassociated_token_returns_not_found() {
    with_contract(|env| {
        assert_eq!(
            nft_collection::get_nft_collection(env, 888),
            Err(Error::TokenNotFound)
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// 12. Event Emission Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn event_emission_direct_publish_mint_and_creator() {
    with_contract(|env| {
        env.ledger().set_timestamp(1_720_000_000);
        let owner = Address::generate(env);
        let uri = String::from_str(env, "ipfs://QmEventUri");

        mint_event::emit_mint(env, &owner, 1201, 1, &uri);
        creator_event::emit_creator_assigned(env, 1, &owner, 1201, 1_720_000_000);

        let events = env.events().all();
        assert_eq!(events.events().len(), 2);
    });
}

#[test]
fn event_emission_execute_mint_publishes_mint_event() {
    with_contract(|env| {
        let owner = Address::generate(env);
        execute_mint(env, make_request(env, &owner, 1202, 500)).unwrap();

        let events = env.events().all();
        assert!(events.events().len() >= 1);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Minting helper, total supply, and status tests
// ─────────────────────────────────────────────────────────────────────────────

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
        let persisted: u32 = env.storage().instance().get(&DataKey::TotalSupply).unwrap();
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
