//! Comprehensive unit tests for the NFT creation workflow.
//!
//! Resolves issue #663: [Minting] Add Unit Tests for NFT Creation Workflow
//!
//! # Coverage
//! 1.  Successful mint — happy-path end-to-end
//! 2.  Sequential token IDs — monotonically increasing counter
//! 3.  Owner assignment — token data persisted with correct owner
//! 4.  Metadata persistence — URI stored and retrievable
//! 5.  Collection supply update — total supply counter increments
//! 6.  Duplicate clip rejection — ClipAlreadyMinted on second mint
//! 7.  Storage failure handling — invalid inputs rejected pre-write
//! 8.  Atomic transaction rollback — no partial state on failure
//! 9.  Mint response generation — MintSuccessResponse fields are correct

#![cfg(test)]

use clips_nft::{
    clip_id_storage, creator_portfolio, creator_storage, execute_batch_mint, execute_mint,
    mint_validator, owner_portfolio, preview_video_uri, royalty_percentage, royalty_recipient,
    thumbnail_uri, token_storage, total_supply, types::TransactionStatus, wallet_token_index,
    AtomicMintContract, BatchMintRequest, DataKey, Error, MintRequest, Royalty,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String, Vec,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test infrastructure
// ─────────────────────────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn make_request(env: &Env, clip_id: u32) -> MintRequest {
    let owner = Address::generate(env);
    let recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner,
        metadata_uri: String::from_str(env, &alloc::format!("ipfs://QmWorkflow{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient,
            basis_points: 500,
            asset_address: None,
        },
        creator_address: None,
        creator_display_name: None,
    }
}

fn make_request_with_owner(env: &Env, owner: &Address, clip_id: u32) -> MintRequest {
    let recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, &alloc::format!("ipfs://QmOwner{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient,
            basis_points: 300,
            asset_address: None,
        },
        creator_address: None,
        creator_display_name: None,
    }
}

fn make_request_full(
    env: &Env,
    owner: &Address,
    creator: &Address,
    clip_id: u32,
    bps: u32,
) -> MintRequest {
    let recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: creator.clone(),
        metadata_uri: String::from_str(env, &alloc::format!("ipfs://QmFull{}", clip_id)),
        thumbnail_uri: Some(String::from_str(
            env,
            &alloc::format!("ipfs://QmThumb{}", clip_id),
        )),
        preview_video_uri: Some(String::from_str(
            env,
            &alloc::format!("ipfs://QmPreview{}", clip_id),
        )),
        royalty_info: Royalty {
            recipient,
            basis_points: bps,
            asset_address: None,
        },
        creator_address: Some(creator.clone()),
        creator_display_name: Some(String::from_str(env, "Test Creator")),
    }
}

fn batch_req(env: &Env, owner: &Address, clip_id: u32) -> MintRequest {
    let recipient = Address::generate(env);
    MintRequest {
        clip_id,
        owner: owner.clone(),
        creator: owner.clone(),
        metadata_uri: String::from_str(env, &alloc::format!("ipfs://QmBatch{}", clip_id)),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipient,
            basis_points: 500,
            asset_address: None,
        },
        creator_address: None,
        creator_display_name: None,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. SUCCESSFUL MINT — happy-path end-to-end
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn successful_mint_returns_ok() {
    with_contract(|env| {
        assert!(execute_mint(env, make_request(env, 1)).is_ok());
    });
}

#[test]
fn successful_mint_status_is_success() {
    with_contract(|env| {
        let result = execute_mint(env, make_request(env, 2)).unwrap();
        assert_eq!(result.status, TransactionStatus::Success);
    });
}

#[test]
fn successful_mint_with_all_optional_fields() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        assert!(execute_mint(env, make_request_full(env, &owner, &creator, 3, 750)).is_ok());
    });
}

#[test]
fn successful_mint_at_max_royalty_bps() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = MintRequest {
            clip_id: 4,
            owner: owner.clone(),
            creator: owner,
            metadata_uri: String::from_str(env, "ipfs://QmMaxRoyalty"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: Royalty {
                recipient: Address::generate(env),
                basis_points: 10_000,
                asset_address: None,
            },
            creator_address: None,
            creator_display_name: None,
        };
        assert!(execute_mint(env, req).is_ok());
    });
}

#[test]
fn successful_mint_with_zero_royalty_bps() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = MintRequest {
            clip_id: 5,
            owner: owner.clone(),
            creator: owner,
            metadata_uri: String::from_str(env, "ipfs://QmZeroRoyalty"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: Royalty {
                recipient: Address::generate(env),
                basis_points: 0,
                asset_address: None,
            },
            creator_address: None,
            creator_display_name: None,
        };
        assert!(execute_mint(env, req).is_ok());
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. SEQUENTIAL TOKEN IDs — monotonically increasing counter
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn first_mint_assigns_token_id_one() {
    with_contract(|env| {
        let result = execute_mint(env, make_request(env, 100)).unwrap();
        assert_eq!(result.token_id, 1);
    });
}

#[test]
fn sequential_mints_produce_strictly_incrementing_ids() {
    with_contract(|env| {
        let ids: alloc::vec::Vec<u32> = (101u32..=105)
            .map(|c| execute_mint(env, make_request(env, c)).unwrap().token_id)
            .collect();
        assert_eq!(ids, alloc::vec![1, 2, 3, 4, 5]);
    });
}

#[test]
fn all_minted_token_ids_are_unique() {
    with_contract(|env| {
        let mut ids: alloc::vec::Vec<u32> = (200u32..=209)
            .map(|c| execute_mint(env, make_request(env, c)).unwrap().token_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    });
}

#[test]
fn token_id_counter_independent_of_clip_id_values() {
    with_contract(|env| {
        let r1 = execute_mint(env, make_request(env, 9999)).unwrap();
        let r2 = execute_mint(env, make_request(env, 1)).unwrap();
        let r3 = execute_mint(env, make_request(env, 500)).unwrap();
        assert_eq!((r1.token_id, r2.token_id, r3.token_id), (1, 2, 3));
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. OWNER ASSIGNMENT — token data persisted with correct owner
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn owner_is_persisted_in_token_data() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request_with_owner(env, &owner, 300)).unwrap();
        let token = token_storage::get_token(env, result.token_id).unwrap();
        assert_eq!(token.owner, owner);
    });
}

#[test]
fn clip_id_is_persisted_in_token_data() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request_with_owner(env, &owner, 301)).unwrap();
        let token = token_storage::get_token(env, result.token_id).unwrap();
        assert_eq!(token.clip_id, 301u32);
    });
}

#[test]
fn multiple_owners_each_hold_correct_token() {
    with_contract(|env| {
        let alice = Address::generate(env);
        let bob = Address::generate(env);
        let ra = execute_mint(env, make_request_with_owner(env, &alice, 310)).unwrap();
        let rb = execute_mint(env, make_request_with_owner(env, &bob, 311)).unwrap();
        assert_eq!(
            token_storage::get_token(env, ra.token_id).unwrap().owner,
            alice
        );
        assert_eq!(
            token_storage::get_token(env, rb.token_id).unwrap().owner,
            bob
        );
    });
}

#[test]
fn single_owner_can_hold_multiple_tokens() {
    with_contract(|env| {
        let owner = Address::generate(env);
        for clip in [320u32, 321, 322] {
            let r = execute_mint(env, make_request_with_owner(env, &owner, clip)).unwrap();
            assert_eq!(
                token_storage::get_token(env, r.token_id).unwrap().owner,
                owner
            );
        }
    });
}

#[test]
fn token_exists_reflects_mint_state() {
    with_contract(|env| {
        assert!(!token_storage::token_exists(env, 1));
        execute_mint(env, make_request(env, 330)).unwrap();
        assert!(token_storage::token_exists(env, 1));
        assert!(!token_storage::token_exists(env, 2));
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. METADATA PERSISTENCE — URI stored and retrievable
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn metadata_uri_is_persisted() {
    with_contract(|env| {
        let req = make_request(env, 400);
        let expected = req.metadata_uri.clone();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            token_storage::get_metadata(env, result.token_id).unwrap(),
            expected
        );
    });
}

#[test]
fn response_metadata_uri_matches_stored_uri() {
    with_contract(|env| {
        let req = make_request(env, 401);
        let expected = req.metadata_uri.clone();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(result.metadata_uri, expected);
    });
}

#[test]
fn thumbnail_uri_is_persisted_when_provided() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let req = make_request_full(env, &owner, &creator, 410, 500);
        let expected = req.thumbnail_uri.clone().unwrap();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            thumbnail_uri::get_thumbnail_uri(env, result.token_id),
            Some(expected)
        );
    });
}

#[test]
fn preview_video_uri_is_persisted_when_provided() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let req = make_request_full(env, &owner, &creator, 411, 500);
        let expected = req.preview_video_uri.clone().unwrap();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            preview_video_uri::get_preview_video_uri(env, result.token_id),
            Some(expected)
        );
    });
}

#[test]
fn thumbnail_uri_absent_when_not_provided() {
    with_contract(|env| {
        let result = execute_mint(env, make_request(env, 420)).unwrap();
        assert_eq!(thumbnail_uri::get_thumbnail_uri(env, result.token_id), None);
    });
}

#[test]
fn preview_video_uri_absent_when_not_provided() {
    with_contract(|env| {
        let result = execute_mint(env, make_request(env, 421)).unwrap();
        assert_eq!(
            preview_video_uri::get_preview_video_uri(env, result.token_id),
            None
        );
    });
}

#[test]
fn royalty_config_is_persisted() {
    with_contract(|env| {
        let req = make_request(env, 430);
        let expected_bps = req.royalty_info.basis_points;
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            token_storage::get_royalty(env, result.token_id)
                .unwrap()
                .basis_points,
            expected_bps
        );
    });
}

#[test]
fn royalty_percentage_index_is_set() {
    with_contract(|env| {
        let req = make_request(env, 431);
        let expected_bps = req.royalty_info.basis_points;
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            royalty_percentage::get_royalty_percentage(env, result.token_id).unwrap(),
            expected_bps
        );
    });
}

#[test]
fn royalty_recipient_index_is_set() {
    with_contract(|env| {
        let req = make_request(env, 432);
        let expected_recipient = req.royalty_info.recipient.clone();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(
            royalty_recipient::get_royalty_recipient(env, result.token_id).unwrap(),
            expected_recipient
        );
    });
}

#[test]
fn metadata_is_scoped_per_token() {
    with_contract(|env| {
        let r1 = execute_mint(env, make_request(env, 440)).unwrap();
        let r2 = execute_mint(env, make_request(env, 441)).unwrap();
        assert_ne!(
            token_storage::get_metadata(env, r1.token_id).unwrap(),
            token_storage::get_metadata(env, r2.token_id).unwrap()
        );
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. COLLECTION SUPPLY UPDATE — total supply and portfolio indexes
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn total_supply_starts_at_zero() {
    with_contract(|env| {
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

#[test]
fn total_supply_increments_by_one_per_mint() {
    with_contract(|env| {
        execute_mint(env, make_request(env, 500)).unwrap();
        assert_eq!(total_supply::get_total_supply(env), 1);
        execute_mint(env, make_request(env, 501)).unwrap();
        assert_eq!(total_supply::get_total_supply(env), 2);
        execute_mint(env, make_request(env, 502)).unwrap();
        assert_eq!(total_supply::get_total_supply(env), 3);
    });
}

#[test]
fn total_supply_matches_number_of_minted_tokens() {
    with_contract(|env| {
        for clip in 510u32..517 {
            execute_mint(env, make_request(env, clip)).unwrap();
        }
        assert_eq!(total_supply::get_total_supply(env), 7);
    });
}

#[test]
fn wallet_index_contains_token_after_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request_with_owner(env, &owner, 520)).unwrap();
        assert!(wallet_token_index::get_wallet_tokens(env, &owner).contains(&result.token_id));
    });
}

#[test]
fn wallet_index_accumulates_multiple_tokens() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let r1 = execute_mint(env, make_request_with_owner(env, &owner, 530)).unwrap();
        let r2 = execute_mint(env, make_request_with_owner(env, &owner, 531)).unwrap();
        let r3 = execute_mint(env, make_request_with_owner(env, &owner, 532)).unwrap();
        let tokens = wallet_token_index::get_wallet_tokens(env, &owner);
        assert!(tokens.contains(&r1.token_id));
        assert!(tokens.contains(&r2.token_id));
        assert!(tokens.contains(&r3.token_id));
        assert_eq!(tokens.len(), 3);
    });
}

#[test]
fn owner_portfolio_contains_token_after_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request_with_owner(env, &owner, 540)).unwrap();
        assert!(owner_portfolio::owner_contains_token(
            env,
            &owner,
            result.token_id
        ));
    });
}

#[test]
fn creator_portfolio_contains_token_after_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let result = execute_mint(env, make_request_full(env, &owner, &creator, 550, 500)).unwrap();
        assert!(creator_portfolio::creator_contains_token(
            env,
            &creator,
            result.token_id
        ));
    });
}

#[test]
fn owner_used_as_default_creator_for_portfolio() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request_with_owner(env, &owner, 551)).unwrap();
        assert!(creator_portfolio::creator_contains_token(
            env,
            &owner,
            result.token_id
        ));
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// CREATOR ASSIGNMENT — persisted at mint time
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn creator_address_is_persisted_after_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let result = execute_mint(env, make_request_full(env, &owner, &creator, 560, 500)).unwrap();
        assert_eq!(
            creator_storage::get_creator(env, result.token_id).unwrap(),
            creator
        );
    });
}

#[test]
fn owner_is_default_creator_when_no_creator_address_given() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let result = execute_mint(env, make_request_with_owner(env, &owner, 561)).unwrap();
        assert_eq!(
            creator_storage::get_creator(env, result.token_id).unwrap(),
            owner
        );
    });
}

#[test]
fn creator_display_name_is_persisted_when_provided() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let display_name = Some(String::from_str(env, "ClipMaster"));
        let req = MintRequest {
            clip_id: 570,
            owner: owner.clone(),
            creator: creator.clone(),
            metadata_uri: String::from_str(env, "ipfs://QmCreatorName"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: Royalty {
                recipient: Address::generate(env),
                basis_points: 500,
                asset_address: None,
            },
            creator_address: Some(creator.clone()),
            creator_display_name: display_name.clone(),
        };
        let result = execute_mint(env, req).unwrap();
        let stored = creator_storage::get_creator_display_name(env, result.token_id).unwrap();
        assert_eq!(stored, display_name);
    });
}

#[test]
fn creator_verified_flag_defaults_to_false_at_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let result = execute_mint(env, make_request_full(env, &owner, &creator, 571, 500)).unwrap();
        assert!(!creator_storage::is_creator_verified(env, result.token_id).unwrap());
    });
}

#[test]
fn creator_query_for_unminted_token_returns_not_found() {
    with_contract(|env| {
        assert_eq!(
            creator_storage::get_creator(env, 9999),
            Err(Error::TokenNotFound)
        );
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. DUPLICATE CLIP REJECTION — ClipAlreadyMinted guard
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn duplicate_clip_id_returns_clip_already_minted() {
    with_contract(|env| {
        execute_mint(env, make_request(env, 600)).unwrap();
        assert_eq!(
            execute_mint(env, make_request(env, 600)),
            Err(Error::ClipAlreadyMinted)
        );
    });
}

#[test]
fn failed_duplicate_does_not_increment_supply() {
    with_contract(|env| {
        execute_mint(env, make_request(env, 601)).unwrap();
        let _ = execute_mint(env, make_request(env, 601));
        assert_eq!(total_supply::get_total_supply(env), 1);
    });
}

#[test]
fn failed_duplicate_does_not_advance_token_id() {
    with_contract(|env| {
        let r1 = execute_mint(env, make_request(env, 602)).unwrap();
        let _ = execute_mint(env, make_request(env, 602));
        let r3 = execute_mint(env, make_request(env, 603)).unwrap();
        assert_eq!(r1.token_id, 1);
        assert_eq!(r3.token_id, 2);
    });
}

#[test]
fn clip_id_minted_sentinel_is_set_after_mint() {
    with_contract(|env| {
        execute_mint(env, make_request(env, 610)).unwrap();
        assert!(clip_id_storage::is_clip_mapped(env, 610));
    });
}

#[test]
fn token_to_clip_id_reverse_mapping_is_recorded() {
    with_contract(|env| {
        let result = execute_mint(env, make_request(env, 611)).unwrap();
        assert_eq!(
            clip_id_storage::get_clip_id(env, result.token_id).unwrap(),
            611u32
        );
    });
}

#[test]
fn distinct_clip_ids_all_mint_successfully() {
    with_contract(|env| {
        for clip in [620u32, 621, 622, 623, 624] {
            assert!(execute_mint(env, make_request(env, clip)).is_ok());
        }
        assert_eq!(total_supply::get_total_supply(env), 5);
    });
}

#[test]
fn validate_mint_detects_duplicate_clip_id() {
    with_contract(|env| {
        execute_mint(env, make_request(env, 630)).unwrap();
        let royalty = Royalty {
            recipient: Address::generate(env),
            basis_points: 500,
            asset_address: None,
        };
        assert_eq!(
            mint_validator::validate_mint(
                env,
                630,
                &String::from_str(env, "ipfs://QmAnother"),
                &royalty,
                &Address::generate(env)
            ),
            Err(Error::ClipAlreadyMinted)
        );
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 7. STORAGE FAILURE HANDLING — invalid inputs rejected pre-write
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn empty_metadata_uri_is_rejected() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = MintRequest {
            clip_id: 700,
            owner: owner.clone(),
            creator: owner,
            metadata_uri: String::from_str(env, ""),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: Royalty {
                recipient: Address::generate(env),
                basis_points: 500,
                asset_address: None,
            },
            creator_address: None,
            creator_display_name: None,
        };
        assert_eq!(execute_mint(env, req), Err(Error::InvalidURI));
    });
}

#[test]
fn failed_empty_uri_mint_leaves_no_storage() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = MintRequest {
            clip_id: 701,
            owner: owner.clone(),
            creator: owner,
            metadata_uri: String::from_str(env, ""),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: Royalty {
                recipient: Address::generate(env),
                basis_points: 500,
                asset_address: None,
            },
            creator_address: None,
            creator_display_name: None,
        };
        let _ = execute_mint(env, req);
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!clip_id_storage::is_clip_mapped(env, 701));
        assert!(!token_storage::token_exists(env, 1));
    });
}

#[test]
fn royalty_bps_above_max_is_rejected() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let req = MintRequest {
            clip_id: 710,
            owner: owner.clone(),
            creator: owner,
            metadata_uri: String::from_str(env, "ipfs://QmHighRoyalty"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: Royalty {
                recipient: Address::generate(env),
                basis_points: 10_001,
                asset_address: None,
            },
            creator_address: None,
            creator_display_name: None,
        };
        assert_eq!(execute_mint(env, req), Err(Error::InvalidBasisPoints));
    });
}

#[test]
fn blacklisted_owner_cannot_mint() {
    with_contract(|env| {
        let owner = Address::generate(env);
        env.storage()
            .persistent()
            .set(&DataKey::Blacklisted(owner.clone()), &true);
        let err = execute_mint(env, make_request_with_owner(env, &owner, 720))
            .expect_err("blacklisted owner must be rejected");
        assert_eq!(err, Error::Unauthorized);
    });
}

#[test]
fn blacklisted_owner_rejection_leaves_no_storage() {
    with_contract(|env| {
        let owner = Address::generate(env);
        env.storage()
            .persistent()
            .set(&DataKey::Blacklisted(owner.clone()), &true);
        let _ = execute_mint(env, make_request_with_owner(env, &owner, 721));
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
    });
}

#[test]
fn validate_mint_rejects_empty_uri() {
    with_contract(|env| {
        let royalty = Royalty {
            recipient: Address::generate(env),
            basis_points: 500,
            asset_address: None,
        };
        assert_eq!(
            mint_validator::validate_mint(
                env,
                730,
                &String::from_str(env, ""),
                &royalty,
                &Address::generate(env)
            ),
            Err(Error::InvalidURI)
        );
        assert_eq!(total_supply::get_total_supply(env), 0);
    });
}

#[test]
fn validate_mint_rejects_excessive_royalty_bps() {
    with_contract(|env| {
        let royalty = Royalty {
            recipient: Address::generate(env),
            basis_points: 10_001,
            asset_address: None,
        };
        assert_eq!(
            mint_validator::validate_mint(
                env,
                731,
                &String::from_str(env, "ipfs://QmOk"),
                &royalty,
                &Address::generate(env)
            ),
            Err(Error::InvalidBasisPoints)
        );
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 8. ATOMIC TRANSACTION ROLLBACK — no partial state on failure
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn valid_batch_mints_all_tokens() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    batch_req(env, &owner, 800),
                    batch_req(env, &owner, 801),
                    batch_req(env, &owner, 802),
                ],
            ),
        };
        let results = execute_batch_mint(env, &batch).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(total_supply::get_total_supply(env), 3);
    });
}

#[test]
fn batch_with_invalid_uri_aborts_with_zero_mints() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let mut bad = batch_req(env, &owner, 811);
        bad.metadata_uri = String::from_str(env, "");
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [batch_req(env, &owner, 810), bad]),
        };
        assert_eq!(execute_batch_mint(env, &batch), Err(Error::InvalidURI));
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
        assert!(!clip_id_storage::is_clip_mapped(env, 810));
        assert_eq!(wallet_token_index::get_wallet_tokens(env, &owner).len(), 0);
    });
}

#[test]
fn batch_with_duplicate_clip_id_rolls_back_all_state() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    batch_req(env, &owner, 820),
                    batch_req(env, &owner, 821),
                    batch_req(env, &owner, 820), // duplicate
                ],
            ),
        };
        assert_eq!(
            execute_batch_mint(env, &batch),
            Err(Error::ClipAlreadyMinted)
        );
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
        assert!(!token_storage::token_exists(env, 2));
        assert_eq!(wallet_token_index::get_wallet_tokens(env, &owner).len(), 0);
    });
}

#[test]
fn batch_mid_execution_failure_rolls_back_prior_items() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        let mut req1 = batch_req(env, &owner, 830);
        req1.creator_address = Some(creator.clone());
        let mut req2 = batch_req(env, &owner, 831);
        req2.creator_address = Some(creator.clone());
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req1, req2]),
        };
        // Pre-register clip 831 to force a ClipAlreadyMinted failure on the second item
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(831u32), &true);
        assert_eq!(
            execute_batch_mint(env, &batch),
            Err(Error::ClipAlreadyMinted)
        );
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!token_storage::token_exists(env, 1));
        assert!(!clip_id_storage::is_clip_mapped(env, 830));
        assert!(!owner_portfolio::owner_contains_token(env, &owner, 1));
        assert!(!wallet_token_index::get_wallet_tokens(env, &owner).contains(&1));
        assert!(!creator_portfolio::creator_contains_token(env, &creator, 1));
    });
}

#[test]
fn single_item_batch_failure_leaves_clean_state() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let mut req = batch_req(env, &owner, 840);
        req.royalty_info.basis_points = 99_999;
        let batch = BatchMintRequest {
            requests: Vec::from_array(env, [req]),
        };
        assert!(execute_batch_mint(env, &batch).is_err());
        assert_eq!(total_supply::get_total_supply(env), 0);
        assert!(!clip_id_storage::is_clip_mapped(env, 840));
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// 9. MINT RESPONSE GENERATION — MintSuccessResponse fields are correct
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn response_token_id_is_correct() {
    with_contract(|env| {
        assert_eq!(
            execute_mint(env, make_request(env, 900)).unwrap().token_id,
            1
        );
    });
}

#[test]
fn response_owner_matches_request_owner() {
    with_contract(|env| {
        let owner = Address::generate(env);
        assert_eq!(
            execute_mint(env, make_request_with_owner(env, &owner, 901))
                .unwrap()
                .owner,
            owner
        );
    });
}

#[test]
fn response_clip_id_matches_request_clip_id() {
    with_contract(|env| {
        assert_eq!(
            execute_mint(env, make_request(env, 902)).unwrap().clip_id,
            902u32
        );
    });
}

#[test]
fn response_metadata_uri_matches_request() {
    with_contract(|env| {
        let req = make_request(env, 903);
        let expected = req.metadata_uri.clone();
        assert_eq!(execute_mint(env, req).unwrap().metadata_uri, expected);
    });
}

#[test]
fn response_status_is_always_success() {
    with_contract(|env| {
        for clip in [910u32, 911, 912] {
            assert_eq!(
                execute_mint(env, make_request(env, clip)).unwrap().status,
                TransactionStatus::Success
            );
        }
    });
}

#[test]
fn response_mint_timestamp_reflects_ledger_time() {
    with_contract(|env| {
        env.ledger().set_timestamp(1_700_000_000);
        assert_eq!(
            execute_mint(env, make_request(env, 920))
                .unwrap()
                .mint_timestamp,
            1_700_000_000
        );
    });
}

#[test]
fn response_timestamps_differ_across_ledger_advances() {
    with_contract(|env| {
        env.ledger().set_timestamp(1_000);
        let r1 = execute_mint(env, make_request(env, 921)).unwrap();
        env.ledger().set_timestamp(2_000);
        let r2 = execute_mint(env, make_request(env, 922)).unwrap();
        assert_eq!(r1.mint_timestamp, 1_000);
        assert_eq!(r2.mint_timestamp, 2_000);
    });
}

#[test]
fn response_all_fields_populated_for_full_request() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        env.ledger().set_timestamp(9_999_999);
        let req = make_request_full(env, &owner, &creator, 930, 750);
        let expected_uri = req.metadata_uri.clone();
        let result = execute_mint(env, req).unwrap();
        assert_eq!(result.token_id, 1);
        assert_eq!(result.owner, owner);
        assert_eq!(result.clip_id, 930u32);
        assert_eq!(result.metadata_uri, expected_uri);
        assert_eq!(result.mint_timestamp, 9_999_999);
        assert_eq!(result.status, TransactionStatus::Success);
    });
}

#[test]
fn batch_results_contain_one_response_per_request_in_order() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let clips = [940u32, 941, 942];
        let batch = BatchMintRequest {
            requests: Vec::from_array(
                env,
                [
                    batch_req(env, &owner, clips[0]),
                    batch_req(env, &owner, clips[1]),
                    batch_req(env, &owner, clips[2]),
                ],
            ),
        };
        let results = execute_batch_mint(env, &batch).unwrap();
        assert_eq!(results.len(), 3);
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.clip_id, clips[i]);
            assert_eq!(result.status, TransactionStatus::Success);
            assert_eq!(result.token_id, (i as u32) + 1);
        }
    });
}
