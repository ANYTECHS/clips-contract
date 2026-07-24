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
    clip_id_storage, creator_portfolio, creator_storage, media_uri_storage, mint_event,
    minted_clip_index,
    mint_request::MintRequest,
    owner_portfolio, royalty_percentage, token_storage, total_supply,
    types::{
        DataKey, Error, MintSuccessResponse, TokenData, TokenId, TransactionStatus,
    },
    wallet_token_index,
};

/// Alias retained for callers that still import [`MintResult`].
pub type MintResult = MintSuccessResponse;

// ─── Optional media attachments on a mint ─────────────────────────────────────

/// Optional thumbnail / preview URIs supplied with a mint.
#[contracttype]
#[derive(Clone)]
pub struct MintMedia {
    pub thumbnail_uri: Option<String>,
    pub preview_uri: Option<String>,
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Orchestrate creation of a new NFT from a validated [`MintRequest`].
pub fn execute_mint(env: &Env, request: MintRequest) -> Result<MintSuccessResponse, Error> {
    execute_mint_with_media(env, request, None, None)
}

/// Like [`execute_mint`], but also persists optional thumbnail / preview URIs.
pub fn execute_mint_with_media(
    env: &Env,
    request: MintRequest,
    thumbnail_uri: Option<String>,
    preview_uri: Option<String>,
) -> Result<MintSuccessResponse, Error> {
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

    // Creator is the mint owner at creation time.
    creator_storage::assign_creator(env, token_id, &request.owner, request.clip_id)?;
    creator_portfolio::add_token_to_creator(env, &request.owner, token_id)?;
    owner_portfolio::add_token_to_owner(env, &request.owner, token_id)?;

    clip_id_storage::save_clip_id(env, token_id, request.clip_id)?;
    minted_clip_index::add_clip(env, request.clip_id)?;

    wallet_token_index::add_token_to_wallet(env, &request.owner, token_id)?;

    increment_token_id(env);
    total_supply::increment_total_supply(env)?;

    let mint_timestamp = env.ledger().timestamp();

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
            owner,
            metadata_uri: String::from_str(env, "ipfs://QmClip"),
            royalty_info: Royalty {
                recipient: royalty_recipient,
                basis_points: 500,
                asset_address: None,
            },
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
        with_contract(|env| {
            let owner = Address::generate(env);
            let recipient = Address::generate(env);
            let req = MintRequest {
                clip_id: 5,
                owner,
                metadata_uri: String::from_str(env, ""),
                royalty_info: Royalty {
                    recipient,
                    basis_points: 0,
                    asset_address: None,
                },
            };
            let err = execute_mint(env, req).expect_err("empty uri should fail");
            assert_eq!(err, Error::InvalidURI);
        });
    }

    #[test]
    fn mint_emits_mint_and_creator_events() {
        with_contract(|env| {
            execute_mint(env, make_request(env, 7)).unwrap();
            let events = env.events().all();
            assert!(
                events.events().len() >= 2,
                "mint + creator events expected"
            );
        });
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
