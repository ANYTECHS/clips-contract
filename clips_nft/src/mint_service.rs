//! Mint service — core orchestration layer for NFT creation.
//!
//! This module is the single entry point for all state mutations during a mint.
//! It assumes upstream callers have already performed authentication and
//! signature verification; this layer focuses on assembling on-chain state.
//!
//! # Responsibilities
//! 1. Reserve the next [`TokenId`].
//! 2. Write owner / clip token data.
//! 3. Write metadata URI (and optional media URIs).
//! 4. Write royalty configuration + percentage.
//! 5. Assign creator and emit creator-assignment event.
//! 6. Record clip_id ↔ token_id mapping and indexes.
//! 7. Increment total supply (overflow-safe).
//! 8. Emit the `"mint"` event.
//! 9. Return a standardized [`MintSuccessResponse`].

use soroban_sdk::{contracttype, Env, String};

use crate::{
    clip_id_storage, creator_portfolio, creator_storage, mint_event, minted_clip_index,
    clip_id_storage, creator_storage, mint_event, minted_clip_index,
    mint_request::MintRequest,
    preview_video_uri, royalty_recipient, thumbnail_uri,
    token_storage,
    types::{DataKey, Error, TokenData, TokenId},
    wallet_token_index,
};

/// Alias retained for callers that still import [`MintResult`].
pub type MintResult = MintSuccessResponse;

// ─── Optional media attachments on a mint ─────────────────────────────────────

/// Optional thumbnail / preview URIs supplied with a mint.
#[contracttype]
#[derive(Clone, Debug)]
pub struct MintResult {
    /// The on-chain token ID assigned to this NFT (auto-incremented).
    pub token_id: TokenId,
    /// The owner address that was recorded for this token.
    pub owner: Address,
    /// The off-chain clip identifier linked to this token.
    pub clip_id: u32,
    /// The metadata URI stored on-chain for this token.
    pub metadata_uri: String,
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Orchestrate the creation of a new NFT from a validated [`MintRequest`].
///
/// All storage writes are performed inside this function; no state change
/// happens before it is called and all mutations are atomic within the
/// Soroban invocation.
///
/// # Errors
/// - [`Error::ClipAlreadyMinted`] — `clip_id` is already mapped to a token.
/// - [`Error::InvalidURI`] — `metadata_uri` is empty (propagated from
///   `token_storage::set_metadata`).
/// - [`Error::EmptyCreator`] — `creator` validation fails (issue #665).
pub fn execute_mint(env: &Env, request: MintRequest) -> Result<MintResult, Error> {
    // 1. Reserve the next token ID before any writes so the ID is stable for
    //    the remaining operations in this invocation.
    let token_id = next_token_id(env);

    let token_data = TokenData {
        owner: request.owner.clone(),
        clip_id: request.clip_id,
    };
    token_storage::set_token(env, token_id, &token_data);

    if request.metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    token_storage::set_metadata(env, token_id, &request.metadata_uri)?;

    if let Some(ref thumb) = thumbnail_uri {
        media_uri_storage::set_thumbnail(env, token_id, thumb);
    }
    if let Some(ref preview) = preview_uri {
        media_uri_storage::set_preview_uri(env, token_id, preview);
    }

    token_storage::set_royalty(env, token_id, &request.royalty_info);
    royalty_percentage::set_royalty_percentage(
        env,
        token_id,
        request.royalty_info.basis_points,
    )?;

    // 4b. Record creator metadata.
    //     If creator_address is provided use it; otherwise default to the owner.
    //     Verified flag defaults to false (only platform can mark as verified).
    let creator_addr = request.creator_address.clone().unwrap_or_else(|| request.owner.clone());
    creator_storage::set_creator_with_name(
        env,
        token_id,
        &creator_addr,
        request.creator_display_name.clone(),
    );

    // 4c. Add token to the creator's portfolio index (issue #674).
    creator_portfolio::add_token_to_creator(env, &creator_addr, token_id).ok();

    // 5. Record the bidirectional clip_id ↔ token_id mapping.
    // 5. Persist the royalty recipient mapping (issue #672).
    //    Stores the first recipient's address for lightweight lookups.
    royalty_recipient::set_royalty_recipient(
        env,
        token_id,
        &request.royalty_info.recipient,
    );

    // 6. Record the bidirectional clip_id ↔ token_id mapping.
    //    Also acts as the duplicate-mint guard (Err(ClipAlreadyMinted) if
    //    clip_id was already registered).
    clip_id_storage::save_clip_id(env, token_id, request.clip_id)?;

    // 7. Mark the clip as minted in the existence index.
    //    We deliberately call this *after* save_clip_id so that any
    //    ClipAlreadyMinted error fires from the canonical dedup guard first.
    minted_clip_index::add_clip(env, request.clip_id)?;

    // 8. Append the token to the owner's wallet index.
    wallet_token_index::add_token_to_wallet(env, &request.owner, token_id);

    // 9. Persist the original creator address (issue #665).
    creator_storage::set_creator(env, token_id, &request.creator)?;

    // 10. Persist the optional thumbnail URI (issue #668).
    if let Some(ref thumb) = request.thumbnail_uri {
        thumbnail_uri::set_thumbnail_uri(env, token_id, thumb)?;
    }

    // 11. Persist the optional preview video URI (issue #669).
    if let Some(ref preview) = request.preview_video_uri {
        preview_video_uri::set_preview_video_uri(env, token_id, preview)?;
    }

    // 12. Advance the token counter and total supply counters.
    increment_token_id(env);
    total_supply::increment_total_supply(env)?;

    let mint_timestamp = env.ledger().timestamp();

    // 13. Emit the standard mint event for off-chain indexers.
    mint_event::emit_mint(
        env,
        &request.owner,
        request.clip_id,
        token_id,
        &request.metadata_uri,
    );

    Ok(MintSuccessResponse {
        token_id,
        owner: request.owner,
        metadata_uri: request.metadata_uri,
        clip_id: request.clip_id,
        mint_timestamp,
        status: TransactionStatus::Success,
    })
}

// ─── Private helpers ─────────────────────────────────────────────────────────

fn next_token_id(env: &Env) -> TokenId {
    env.storage()
        .instance()
        .get::<DataKey, TokenId>(&DataKey::NextTokenId)
        .unwrap_or(crate::storage_constants::DEFAULT_NEXT_TOKEN_ID)
        .saturating_add(1)
}

fn increment_token_id(env: &Env) {
    let current: TokenId = env
        .storage()
        .instance()
        .get::<DataKey, TokenId>(&DataKey::NextTokenId)
        .unwrap_or(crate::storage_constants::DEFAULT_NEXT_TOKEN_ID);
    env.storage()
        .instance()
        .set(&DataKey::NextTokenId, &current.saturating_add(1));
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use crate::{
        mint_request::MintRequest,
        types::{DataKey, Royalty},
        AtomicMintContract,
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
            creator_address: None,
            creator_display_name: None,
        }
    }

    #[test]
    fn first_mint_assigns_token_id_one() {
        with_contract(|env| {
            let req = make_request(env, 42);
            let result = execute_mint(env, req.clone()).expect("mint should succeed");
            assert_eq!(result.token_id, 1);
            assert_eq!(result.clip_id, 42);
            assert_eq!(result.owner, req.owner);
            assert_eq!(result.metadata_uri, req.metadata_uri);
            assert_eq!(result.status, TransactionStatus::Success);
        });
    }

    #[test]
    fn mint_success_response_includes_timestamp() {
        with_contract(|env| {
            env.ledger().set_timestamp(1_700_000_555);
            let result = execute_mint(env, make_request(env, 1)).unwrap();
            assert_eq!(result.mint_timestamp, 1_700_000_555);
            assert_eq!(result.status, TransactionStatus::Success);
        });
    }

    #[test]
    fn sequential_mints_increment_token_id() {
        with_contract(|env| {
            let r1 = execute_mint(env, make_request(env, 1)).unwrap();
            let r2 = execute_mint(env, make_request(env, 2)).unwrap();
            let r3 = execute_mint(env, make_request(env, 3)).unwrap();
            assert_eq!(r1.token_id, 1);
            assert_eq!(r2.token_id, 2);
            assert_eq!(r3.token_id, 3);
        });
    }

    #[test]
    fn total_supply_increments() {
        with_contract(|env| {
            assert_eq!(total_supply::get_total_supply(env), 0);
            execute_mint(env, make_request(env, 10)).unwrap();
            assert_eq!(total_supply::get_total_supply(env), 1);
            execute_mint(env, make_request(env, 11)).unwrap();
            assert_eq!(total_supply::get_total_supply(env), 2);
        });
    }

    #[test]
    fn duplicate_clip_id_fails() {
        with_contract(|env| {
            execute_mint(env, make_request(env, 99)).unwrap();
            let err = execute_mint(env, make_request(env, 99)).unwrap_err();
            assert_eq!(err, Error::ClipAlreadyMinted);
        });
    }

    #[test]
    fn empty_metadata_uri_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        let req = MintRequest {
            clip_id: 5,
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
            creator_address: None,
            creator_display_name: None,
        };

        let err = execute_mint(&env, req).expect_err("empty uri should fail");
        assert_eq!(err, Error::InvalidURI);
    }

    #[test]
    fn mint_emits_event() {
        let env = Env::default();
        let req = make_request(&env, 7);

        execute_mint(&env, req).expect("mint ok");

        let events = env.events().all();
        assert_eq!(events.iter().count(), 1, "exactly one event should be emitted");
    }

    #[test]
    fn token_storage_has_correct_data() {
        with_contract(|env| {
            let req = make_request(env, 20);
            let owner = req.owner.clone();
            let result = execute_mint(env, req).unwrap();
            let stored = token_storage::get_token(env, result.token_id).unwrap();
            assert_eq!(stored.owner, owner);
            assert_eq!(stored.clip_id, 20);
        });
    }

    #[test]
    fn media_uris_are_persisted() {
        with_contract(|env| {
            let req = make_request(env, 30);
            let thumb = String::from_str(env, "ipfs://thumb");
            let preview = String::from_str(env, "ipfs://preview");
            let result =
                execute_mint_with_media(env, req, Some(thumb.clone()), Some(preview.clone()))
                    .unwrap();
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

    /// The clip_id → token_id mapping is recorded after a successful mint.
    #[test]
    fn clip_id_mapping_is_recorded() {
        let env = Env::default();
        let req = make_request(&env, 55);
        let clip_id = req.clip_id;

        let result = execute_mint(&env, req).expect("mint ok");

        // Forward mapping: clip_id → token_id
        assert!(env
            .storage()
            .persistent()
            .has(&DataKey::ClipIdMinted(clip_id)));

        // Reverse mapping: token_id → clip_id
        let stored_clip = clip_id_storage::get_clip_id(&env, result.token_id)
            .expect("reverse mapping should exist");
        assert_eq!(stored_clip, clip_id);
    }

    /// The token appears in the owner's wallet index after minting.
    #[test]
    fn token_added_to_wallet_index() {
        let env = Env::default();
        let req = make_request(&env, 60);
        let owner = req.owner.clone();

        let result = execute_mint(&env, req).expect("mint ok");

        let tokens = wallet_token_index::get_wallet_tokens(&env, &owner);
        assert!(
            tokens.contains(&result.token_id),
            "token should be in the owner's wallet index"
        );
    }

    /// Creator metadata defaults to owner address when no explicit creator is set.
    #[test]
    fn creator_metadata_defaults_to_owner() {
        let env = Env::default();
        let req = make_request(&env, 70);
        let owner = req.owner.clone();

        let result = execute_mint(&env, req).expect("mint ok");

        let stored_creator = creator_storage::get_creator(&env, result.token_id).unwrap();
        assert_eq!(stored_creator, owner);

        let metadata = creator_storage::get_creator_metadata(&env, result.token_id).unwrap();
        assert_eq!(metadata.creator_address, owner);
        assert_eq!(metadata.display_name, None);
        assert!(!metadata.verified);
    }

    /// Creator metadata uses explicit creator_address and creator_display_name when provided.
    #[test]
    fn creator_metadata_with_explicit_creator_and_name() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);
        let display_name = Some(String::from_str(&env, "ClipCreator"));
        let royalty_recipient = Address::generate(&env);

        let req = MintRequest {
            clip_id: 71,
            owner: owner.clone(),
            metadata_uri: String::from_str(&env, "ipfs://QmExplicitCreator"),
            royalty_info: Royalty {
                recipient: royalty_recipient,
                basis_points: 250,
                asset_address: None,
            },
            creator_address: Some(creator.clone()),
            creator_display_name: display_name.clone(),
        };

        let result = execute_mint(&env, req).expect("mint ok");

        let metadata = creator_storage::get_creator_metadata(&env, result.token_id).unwrap();
        assert_eq!(metadata.creator_address, creator);
        assert_ne!(metadata.creator_address, owner);
        assert_eq!(metadata.display_name, display_name);
        assert!(!metadata.verified);

        let stored_name = creator_storage::get_creator_display_name(&env, result.token_id).unwrap();
        assert_eq!(stored_name, display_name);
    }

    /// Minted token appears in the creator's portfolio index.
    #[test]
    fn token_added_to_creator_portfolio() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let owner = Address::generate(&env);
        let royalty_recipient = Address::generate(&env);

        let req = MintRequest {
            clip_id: 72,
            owner,
            metadata_uri: String::from_str(&env, "ipfs://QmPortfolioTest"),
            royalty_info: Royalty {
                recipient: royalty_recipient,
                basis_points: 500,
                asset_address: None,
            },
            creator_address: Some(creator.clone()),
            creator_display_name: None,
        };

        let result = execute_mint(&env, req).expect("mint ok");

        let portfolio = creator_portfolio::get_creator_portfolio(&env, &creator);
        assert!(portfolio.contains(&result.token_id));
        assert_eq!(portfolio.len(), 1);
    }

    // ── next_token_id helper ─────────────────────────────────────────────────

    /// next_token_id returns 1 when no counter is set yet.
    #[test]
    fn next_token_id_starts_at_one() {
        with_contract(|env| {
            assert_eq!(next_token_id(env), 1);
        });
    }

    #[test]
    fn next_token_id_reads_existing_counter() {
        with_contract(|env| {
            env.storage()
                .instance()
                .set(&DataKey::NextTokenId, &5u32);
            assert_eq!(next_token_id(env), 6);
        });
    }
}
