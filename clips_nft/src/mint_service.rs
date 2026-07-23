//! Mint service — core orchestration layer for NFT creation.
//!
//! Resolves issue #651. This module is the single entry point for all state
//! mutations during a mint. It assumes that upstream callers have already
//! performed authentication and signature verification; this layer focuses
//! purely on assembling the on-chain state after validation passes.
//!
//! # Responsibilities
//! 1. Read and reserve the next available [`TokenId`].
//! 2. Write [`TokenData`] (owner + clip_id) to persistent storage.
//! 3. Write the metadata URI to persistent storage.
//! 4. Write the royalty configuration to persistent storage.
//! 5. Record the bidirectional clip_id ↔ token_id mapping.
//! 6. Add the token to the owner's wallet index.
//! 7. Increment the global token counter and total supply.
//! 8. Emit the `"mint"` event.
//! 9. Return a [`MintResult`] describing the freshly minted token.
//!
//! # Usage
//! ```rust,ignore
//! use crate::mint_service::{execute_mint, MintResult};
//! use crate::mint_request::MintRequest;
//!
//! let result: MintResult = execute_mint(&env, request)?;
//! ```

use soroban_sdk::{contracttype, Address, Env, String};

use crate::{
    clip_id_storage, mint_event, minted_clip_index,
    mint_request::MintRequest,
    token_storage,
    types::{DataKey, Error, TokenData, TokenId},
    wallet_token_index,
};

// ─── Result type ─────────────────────────────────────────────────────────────

/// Returned by [`execute_mint`] on success.
///
/// Aggregates all minting outputs so callers can forward them to the user
/// without additional storage round-trips.
#[contracttype]
#[derive(Clone)]
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
pub fn execute_mint(env: &Env, request: MintRequest) -> Result<MintResult, Error> {
    // 1. Reserve the next token ID before any writes so the ID is stable for
    //    the remaining operations in this invocation.
    let token_id = next_token_id(env);

    // 2. Write the core token record: owner + clip_id.
    let token_data = TokenData {
        owner: request.owner.clone(),
        clip_id: request.clip_id,
    };
    token_storage::set_token(env, token_id, &token_data);

    // 3. Write the metadata URI. Returns Err(InvalidURI) for empty strings.
    token_storage::set_metadata(env, token_id, &request.metadata_uri)?;

    // 4. Write the royalty configuration.
    token_storage::set_royalty(env, token_id, &request.royalty_info);

    // 5. Record the bidirectional clip_id ↔ token_id mapping.
    //    Also acts as the duplicate-mint guard (Err(ClipAlreadyMinted) if
    //    clip_id was already registered).
    clip_id_storage::save_clip_id(env, token_id, request.clip_id)?;

    // 6. Mark the clip as minted in the existence index.
    //    We deliberately call this *after* save_clip_id so that any
    //    ClipAlreadyMinted error fires from the canonical dedup guard first.
    minted_clip_index::add_clip(env, request.clip_id)?;

    // 7. Append the token to the owner's wallet index.
    wallet_token_index::add_token_to_wallet(env, &request.owner, token_id);

    // 8. Advance the token counter and total supply counters.
    increment_token_id(env);
    increment_total_supply(env);

    // 9. Emit the standard mint event for off-chain indexers.
    mint_event::emit_mint(
        env,
        &request.owner,
        request.clip_id,
        token_id,
        &request.metadata_uri,
    );

    Ok(MintResult {
        token_id,
        owner: request.owner,
        clip_id: request.clip_id,
        metadata_uri: request.metadata_uri,
    })
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// Read the current value of [`DataKey::NextTokenId`] from instance storage.
///
/// Token IDs start at `1` so that `0` can be used as a sentinel "not found"
/// value in external tooling. The first call before any mint returns `1`.
fn next_token_id(env: &Env) -> TokenId {
    env.storage()
        .instance()
        .get::<DataKey, TokenId>(&DataKey::NextTokenId)
        .unwrap_or(crate::storage_constants::DEFAULT_NEXT_TOKEN_ID)
        .saturating_add(1)
}

/// Persist the new [`DataKey::NextTokenId`] value (one higher than what was
/// just assigned).
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

/// Persist the new [`DataKey::TotalSupply`] value.
fn increment_total_supply(env: &Env) {
    let current: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::TotalSupply)
        .unwrap_or(crate::storage_constants::DEFAULT_TOTAL_SUPPLY);
    env.storage()
        .instance()
        .set(&DataKey::TotalSupply, &current.saturating_add(1));
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mint_request::MintRequest,
        types::{DataKey, Royalty},
    };
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, String,
    };

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_request(env: &Env, clip_id: u32) -> MintRequest {
        let owner = Address::generate(env);
        let royalty_recipient = Address::generate(env);
        MintRequest {
            clip_id,
            owner,
            metadata_uri: String::from_str(env, &format!("ipfs://QmClip{}", clip_id)),
            royalty_info: Royalty {
                recipient: royalty_recipient,
                basis_points: 500,
                asset_address: None,
            },
        }
    }

    // ── execute_mint ─────────────────────────────────────────────────────────

    /// First mint assigns token_id = 1 and returns the correct MintResult.
    #[test]
    fn first_mint_assigns_token_id_one() {
        let env = Env::default();
        let req = make_request(&env, 42);

        let result = execute_mint(&env, req.clone()).expect("mint should succeed");

        assert_eq!(result.token_id, 1, "first token id should be 1");
        assert_eq!(result.clip_id, 42);
        assert_eq!(result.owner, req.owner);
        assert_eq!(result.metadata_uri, req.metadata_uri);
    }

    /// Sequential mints assign monotonically increasing token IDs.
    #[test]
    fn sequential_mints_increment_token_id() {
        let env = Env::default();

        let r1 = execute_mint(&env, make_request(&env, 1)).expect("first mint");
        let r2 = execute_mint(&env, make_request(&env, 2)).expect("second mint");
        let r3 = execute_mint(&env, make_request(&env, 3)).expect("third mint");

        assert_eq!(r1.token_id, 1);
        assert_eq!(r2.token_id, 2);
        assert_eq!(r3.token_id, 3);
    }

    /// Total supply increments with each mint.
    #[test]
    fn total_supply_increments() {
        let env = Env::default();

        let supply_before: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        assert_eq!(supply_before, 0);

        execute_mint(&env, make_request(&env, 10)).unwrap();
        let supply_after: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        assert_eq!(supply_after, 1);

        execute_mint(&env, make_request(&env, 11)).unwrap();
        let supply_two: u32 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        assert_eq!(supply_two, 2);
    }

    /// Minting the same clip_id twice returns ClipAlreadyMinted.
    #[test]
    fn duplicate_clip_id_fails() {
        let env = Env::default();

        execute_mint(&env, make_request(&env, 99)).expect("first mint ok");

        let err = execute_mint(&env, make_request(&env, 99))
            .expect_err("duplicate mint should fail");
        assert_eq!(err, Error::ClipAlreadyMinted);
    }

    /// A mint with an empty metadata URI returns InvalidURI.
    #[test]
    fn empty_metadata_uri_fails() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        let req = MintRequest {
            clip_id: 5,
            owner,
            metadata_uri: String::from_str(&env, ""),
            royalty_info: Royalty {
                recipient,
                basis_points: 0,
                asset_address: None,
            },
        };

        let err = execute_mint(&env, req).expect_err("empty uri should fail");
        assert_eq!(err, Error::InvalidURI);
    }

    /// execute_mint emits exactly one "mint" event.
    #[test]
    fn mint_emits_event() {
        let env = Env::default();
        let req = make_request(&env, 7);

        execute_mint(&env, req).expect("mint ok");

        let events = env.events().all();
        assert_eq!(events.len(), 1, "exactly one event should be emitted");
    }

    /// The token data written to storage has the correct owner and clip_id.
    #[test]
    fn token_storage_has_correct_data() {
        let env = Env::default();
        let req = make_request(&env, 20);
        let owner = req.owner.clone();

        let result = execute_mint(&env, req).expect("mint ok");

        let stored = token_storage::get_token(&env, result.token_id)
            .expect("token should exist");
        assert_eq!(stored.owner, owner);
        assert_eq!(stored.clip_id, 20);
    }

    /// The metadata URI written to storage matches the request.
    #[test]
    fn metadata_storage_has_correct_uri() {
        let env = Env::default();
        let req = make_request(&env, 30);
        let expected_uri = req.metadata_uri.clone();

        let result = execute_mint(&env, req).expect("mint ok");

        let stored_uri = token_storage::get_metadata(&env, result.token_id)
            .expect("metadata should exist");
        assert_eq!(stored_uri, expected_uri);
    }

    /// The royalty written to storage matches the request.
    #[test]
    fn royalty_storage_has_correct_data() {
        let env = Env::default();
        let req = make_request(&env, 40);
        let expected_bps = req.royalty_info.basis_points;

        let result = execute_mint(&env, req).expect("mint ok");

        let stored_royalty = token_storage::get_royalty(&env, result.token_id)
            .expect("royalty should exist");
        assert_eq!(stored_royalty.basis_points, expected_bps);
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

    // ── next_token_id helper ─────────────────────────────────────────────────

    /// next_token_id returns 1 when no counter is set yet.
    #[test]
    fn next_token_id_starts_at_one() {
        let env = Env::default();
        assert_eq!(next_token_id(&env), 1);
    }

    /// next_token_id returns the persisted value + 1 when the counter exists.
    #[test]
    fn next_token_id_reads_existing_counter() {
        let env = Env::default();
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &5u32);
        assert_eq!(next_token_id(&env), 6);
    }
}
