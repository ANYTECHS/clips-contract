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

use soroban_sdk::{contracttype, Address, Env, String, Vec};

use crate::{
    batch_id_storage, clip_id_storage, creator_portfolio, creator_storage, mint_event,
    mint_validator, mint_request::{BatchMintRequest, MintRequest}, owner_portfolio,
    preview_video_uri, royalty_assigned_event, royalty_percentage, royalty_recipient,
    thumbnail_uri, token_storage, total_supply,
    types::{
        BatchMintResponse, DataKey, Error, MintSuccessResponse, TokenData, TokenId,
        TransactionStatus,
    },
    wallet_token_index,
};

/// Alias retained for callers that still import [`MintResult`].
pub type MintResult = MintSuccessResponse;

/// Roll back all on-chain effects of a single mint.
///
/// Called from the batch-mint executor when any item in the batch fails after
/// prior items have succeeded, so that the batch remains fully atomic.
///
/// Accepts `&MintSuccessResponse` — the same struct returned by `execute_mint`
/// (and also accessible via the `MintResult` type alias).  All fields required
/// for rollback (`token_id`, `owner`, `clip_id`) are part of the public
/// response struct.
fn revert_single_mint(env: &Env, result: &MintSuccessResponse, creator: &Address) {
    let token_id = result.token_id;
    let clip_id = result.clip_id;
    let owner = &result.owner;

    // 1. Remove wallet token index
    wallet_token_index::remove_token_from_wallet(env, owner, token_id);

    // 1b. Remove owner portfolio entry (written by execute_mint_inner step 7b).
    let mut owner_tokens = owner_portfolio::get_owner_portfolio(env, owner);
    if let Some(pos) = owner_tokens.iter().position(|t| t == token_id) {
        owner_tokens.remove(pos as u32);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(owner.clone()), &owner_tokens);
    }

    // 2. Remove creator portfolio entry
    let mut creator_tokens = creator_portfolio::get_creator_portfolio(env, creator);
    if let Some(pos) = creator_tokens.iter().position(|t| t == token_id) {
        creator_tokens.remove(pos as u32);
        env.storage()
            .persistent()
            .set(&DataKey::CreatorTokens(creator.clone()), &creator_tokens);
    }

    // 3. Remove creator metadata
    creator_storage::remove_creator_metadata(env, token_id);

    // 4. Remove clip id mappings
    env.storage().persistent().remove(&DataKey::TokenClipId(token_id));
    env.storage().persistent().remove(&DataKey::ClipIdMinted(clip_id));

    // 5. Remove media URIs
    env.storage().persistent().remove(&DataKey::ThumbnailUri(token_id));
    env.storage().persistent().remove(&DataKey::PreviewVideoUri(token_id));

    // 6. Remove royalty & metadata
    env.storage().persistent().remove(&DataKey::Royalty(token_id));
    env.storage().persistent().remove(&DataKey::RoyaltyPercentage(token_id));
    env.storage().persistent().remove(&DataKey::RoyaltyRecipient(token_id));
    env.storage().persistent().remove(&DataKey::Metadata(token_id));
    env.storage().persistent().remove(&DataKey::Token(token_id));
}

/// Validate and execute a batch of mint requests atomically.
///
/// Pre-validates EVERY mint request included in the batch before any state
/// write or processing begins. If any request fails during creation or validation,
/// all storage updates are completely rolled back and no partial mints occur.
///
/// Returns a reusable [`BatchMintResponse`] containing the assigned batch ID,
/// minted token IDs, success/failure counts, and processing timestamp.
pub fn execute_batch_mint(
    env: &Env,
    batch: &BatchMintRequest,
) -> Result<BatchMintResponse, Error> {
    // 1. Reserve a unique batch identifier.  This counter is bumped even on
    //    subsequent failures so IDs are never re-used across invocations.
    let batch_id = batch_id_storage::reserve_batch_id(env);

    // 2. Pre-validate every request in the batch before processing begins
    mint_validator::validate_batch_mint(env, batch)?;

    // 3. Track initial counters for rollback safety
    let initial_next_token_id: TokenId = env
        .storage()
        .instance()
        .get(&DataKey::NextTokenId)
        .unwrap_or(crate::storage_constants::DEFAULT_NEXT_TOKEN_ID);
    let initial_total_supply = total_supply::get_total_supply(env);

    let mut results = Vec::new(env);
    let mut creators = Vec::new(env);

    // 4. Execute mints with atomic rollback protection.
    //
    // Cache optimization: maintain per-address portfolio caches so that batches
    // sharing a single owner/creator perform exactly ONE persistent
    // WalletTokens(owner)/CreatorTokens(creator) read + ONE final write
    // per unique address, instead of re-reading and re-writing the
    // entire index vector once per NFT.
    //
    // Each cache is (address, in-memory Vec<TokenId>).  On a batch
    // failure the caches are simply dropped without flushing, so the
    // existing per-token rollback pipeline sees only the pre-batch persisted
    // state (correct: the caches never reached storage yet.
    //
    // Savings per N-item same-owner + same-creator batch:
    //   * WalletTokens  :  reads  N → 1,  writes  N → 1
    //   * CreatorTokens :  reads  N → 1,  writes  N → 1
    let mut active_wallet: Option<(Address, Vec<TokenId>)> = None;
    let mut active_creator: Option<(Address, Vec<TokenId>)> = None;

    for request in batch.requests.iter() {
        let creator_addr = request
            .creator_address
            .clone()
            .unwrap_or_else(|| request.owner.clone());

        // --- Wallet cache: switch to new owner cache + flush previous if different
        let wallet_same = matches!(&active_wallet, Some((w, _)) if w == &request.owner);
        if !wallet_same {
            if let Some((prev_addr, prev_vec)) = active_wallet.take() {
                wallet_token_index::flush_wallet_cache(env, &prev_addr, &prev_vec);
            }
            let loaded = wallet_token_index::get_wallet_tokens(env, &request.owner);
            active_wallet = Some((request.owner.clone(), loaded));
        }

        // --- Creator cache: switch to new creator cache + flush previous if different
        let creator_same = matches!(&active_creator, Some((c, _)) if c == &creator_addr);
        if !creator_same {
            if let Some((prev_addr, prev_vec)) = active_creator.take() {
                creator_portfolio::flush_creator_portfolio_cache(env, &prev_addr, &prev_vec);
            }
            let loaded = creator_portfolio::get_creator_portfolio(env, &creator_addr);
            active_creator = Some((creator_addr.clone(), loaded));
        }

        // Unpack mutable references into the active cache vectors (they are
        // always Some(...) right now because we just set them above).
        // Use &mut Option to avoid moving active_wallet / active_creator.
        let wallet_cache_ref = match &mut active_wallet {
            Some((_, v)) => v,
            None => unreachable!(),
        };
        let creator_cache_ref = match &mut active_creator {
            Some((_, v)) => v,
            None => unreachable!(),
        };

        match execute_mint_inner(
            env,
            request.clone(),
            Some(wallet_cache_ref),
            Some(creator_cache_ref),
            true, // clip uniqueness already verified by validate_batch_mint above
        ) {
            Ok(result) => {
                results.push_back(result);
                creators.push_back(creator_addr);
            }
            Err(err) => {
                // Roll back all prior mints in this batch.
                //
                // Portfolio caches are deliberately NOT flushed here —
                // they are only written to storage after a 100% successful
                // batch (see flush after the loop).  Since this error branch
                // never reaches those flushes, the wallet/creator indexes
                // on disk still contain exactly their pre-batch values.
                // revert_single_mint therefore only needs to clean the
                // per-token persistent keys written inside execute_mint_inner
                // (Token, Metadata, Creator, ClipIdMinted, media URIs, etc.);
                // the rollback paths inside revert for wallet/creator indexes
                // correctly become no-ops for the cache-using batch path
                // (they attempt to remove from vectors that don't yet contain
                // the new token IDs, so zero stale writes occur).
                for i in 0..results.len() {
                    let res = results.get(i).unwrap();
                    let creator = creators.get(i).unwrap();
                    revert_single_mint(env, &res, &creator);
                }

                // Restore counters to pre-batch state
                env.storage()
                    .instance()
                    .set(&DataKey::NextTokenId, &initial_next_token_id);
                total_supply::set_total_supply(env, initial_total_supply);

                return Err(err);
            }
        }
    }

    // Flush final active portfolio caches to storage after a fully successful
    // batch.  On error these caches are simply never written.
    if let Some((addr, vec)) = active_wallet {
        wallet_token_index::flush_wallet_cache(env, &addr, &vec);
    }
    if let Some((addr, vec)) = active_creator {
        creator_portfolio::flush_creator_portfolio_cache(env, &addr, &vec);
    }

    // 5. Aggregate per-token results into the reusable BatchMintResponse.
    //    Current implementation is atomic all-or-nothing, so `failure_count`
    //    is always 0 when `Ok` is returned.  The field is retained here so
    //    future partial-mint modes can populate it without breaking the API.
    let success_count: u32 = results.len().into();
    let mut minted_token_ids: Vec<TokenId> = Vec::with_capacity(env, success_count as u32);
    for r in results.iter() {
        minted_token_ids.push_back(r.token_id);
    }
    let processed_at = env.ledger().timestamp();
    Ok(BatchMintResponse {
        batch_id,
        minted_token_ids,
        success_count,
        failure_count: 0,
        processed_at,
    })
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
    execute_mint_inner(env, request, None, None, false)
}

/// Mint a single NFT with optional separate thumbnail and preview URIs.
///
/// Behaves identically to [`execute_mint`] but accepts media URIs as explicit
/// parameters so callers don't have to populate the `MintRequest` fields
/// directly.
pub fn execute_mint_with_media(
    env: &Env,
    mut request: MintRequest,
    thumbnail: Option<String>,
    preview: Option<String>,
) -> Result<MintResult, Error> {
    if thumbnail.is_some() {
        request.thumbnail_uri = thumbnail;
    }
    if preview.is_some() {
        request.preview_video_uri = preview;
    }
    execute_mint_inner(env, request, None, None, false)
}

/// Internal mint executor that optionally accepts caller-managed wallet and
/// creator portfolio caches.
///
/// When a cache is provided the function only updates it in memory (via
/// [`wallet_token_index::add_token_to_wallet_in_memory`] /
/// [`creator_portfolio::add_token_to_creator_in_memory`]).  The caller is
/// responsible for calling [`wallet_token_index::flush_wallet_cache`] /
/// [`creator_portfolio::flush_creator_portfolio_cache`] once all batch items
/// have succeeded.  On failure the caches are simply discarded, so the
/// existing atomic-rollback pipeline remains correct.
///
/// Passing `None` for either cache falls back to the per-mint
/// read-check-append-write behaviour, preserving the public
/// [`execute_mint`] contract for single-item callers.
///
/// `clip_already_validated` — when `true` (batch path) the clip-ID dedup
/// check inside `save_clip_id` is skipped because `validate_batch_mint` has
/// already performed it.  This saves one persistent `has()` read per item.
fn execute_mint_inner(
    env: &Env,
    request: MintRequest,
    wallet_cache: Option<&mut Vec<TokenId>>,
    creator_cache: Option<&mut Vec<TokenId>>,
    clip_already_validated: bool,
) -> Result<MintResult, Error> {
    // 1. Reserve the next token ID with a single instance-storage read+write.
    let token_id = reserve_token_id(env);

    let token_data = TokenData {
        owner: request.owner.clone(),
        clip_id: request.clip_id,
    };
    token_storage::set_token(env, token_id, &token_data);

    if request.metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    token_storage::set_metadata(env, token_id, &request.metadata_uri)?;

    token_storage::set_royalty(env, token_id, &request.royalty_info);
    royalty_percentage::set_royalty_percentage(
        env,
        token_id,
        request.royalty_info.basis_points,
    )?;

    // 4a-event. Emit royalty-assigned event now that all royalty writes are
    //           complete (issue #695).  Emitted before any further writes so
    //           subscribers know the royalty is persisted even if a later
    //           step panics (Soroban rolls back state but not event queues).
    royalty_assigned_event::emit_royalty_assigned(
        env,
        token_id,
        &request.royalty_info.recipient,
        request.royalty_info.basis_points,
        env.ledger().timestamp(),
    );

    // 4b. Record creator metadata (single write — includes address + display_name).
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
    //
    // Optimization: when the batch caller supplies an in-memory
    // `creator_cache`, extend the cached vector without touching storage.
    // The caller flushes the cache once, at the end of a successful batch.
    match creator_cache {
        Some(c) => {
            creator_portfolio::add_token_to_creator_in_memory(c, token_id).ok();
        }
        None => {
            creator_portfolio::add_token_to_creator(env, &creator_addr, token_id).ok();
        }
    }

    // 5. Persist the royalty recipient mapping (issue #672).
    //    Stores the first recipient's address for lightweight lookups.
    royalty_recipient::set_royalty_recipient(
        env,
        token_id,
        &request.royalty_info.recipient,
    );

    // 6. Record the bidirectional clip_id ↔ token_id mapping.
    //    ClipIdMinted(clip_id) → token_id acts as both the forward mapping and
    //    the dedup guard; no separate ClipMinted existence marker is needed.
    //
    //    Optimization: when the batch path has already validated uniqueness,
    //    skip the redundant `has()` read inside save_clip_id.
    if clip_already_validated {
        clip_id_storage::save_clip_id_unchecked(env, token_id, request.clip_id);
    } else {
        clip_id_storage::save_clip_id(env, token_id, request.clip_id)?;
    }

    // 7. Append the token to the owner's wallet index.
    //
    // Optimization: when the batch caller supplies an in-memory
    // `wallet_cache`, extend the cached vector without touching storage.
    // The caller flushes the cache once, at the end of a successful batch.
    match wallet_cache {
        Some(c) => {
            // Mirror the public API semantics: duplicate entries are
            // ignored (same as the no-cache path discarding the Result).
            wallet_token_index::add_token_to_wallet_in_memory(c, token_id).ok();
        }
        None => {
            let _ = wallet_token_index::add_token_to_wallet(env, &request.owner, token_id);
        }
    }

    // 7b. Update the owner portfolio index (issue #675).
    owner_portfolio::add_token_to_owner(env, &request.owner, token_id).ok();

    // 8. Persist the optional thumbnail URI (issue #668).
    if let Some(ref thumb) = request.thumbnail_uri {
        thumbnail_uri::set_thumbnail_uri(env, token_id, thumb)?;
    }

    // 9. Persist the optional preview video URI (issue #669).
    if let Some(ref preview) = request.preview_video_uri {
        preview_video_uri::set_preview_video_uri(env, token_id, preview)?;
    }

    // 10. Increment total supply.
    total_supply::increment_total_supply(env)?;

    let mint_timestamp = env.ledger().timestamp();

    // 11. Emit the standard mint event for off-chain indexers.
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

/// Read, increment, and persist the next token ID in a single operation.
///
/// Eliminates the previous two-read pattern (separate `next_token_id` read +
/// `increment_token_id` read-then-write) down to a single read + one write.
fn reserve_token_id(env: &Env) -> TokenId {
    let current: TokenId = env
        .storage()
        .instance()
        .get::<DataKey, TokenId>(&DataKey::NextTokenId)
        .unwrap_or(crate::storage_constants::DEFAULT_NEXT_TOKEN_ID);
    let next = current.saturating_add(1);
    env.storage()
        .instance()
        .set(&DataKey::NextTokenId, &next);
    next
}

/// Peek at the next token ID that would be assigned without mutating state.
///
/// Used only in unit tests to assert counter behaviour.
#[cfg(test)]
fn next_token_id(env: &Env) -> TokenId {
    env.storage()
        .instance()
        .get::<DataKey, TokenId>(&DataKey::NextTokenId)
        .unwrap_or(crate::storage_constants::DEFAULT_NEXT_TOKEN_ID)
        .saturating_add(1)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use crate::{
        media_uri_storage,
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
