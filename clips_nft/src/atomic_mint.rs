//! Atomic mint executor — ensures minting is all-or-nothing.
//!
//! # Rollback behaviour
//!
//! Minting is split into two phases:
//!
//! 1. **Validation (read-only)** — signature replay, owner, clip dedup, metadata, royalty.
//! 2. **Write (mutating)** — owner, metadata, royalty, clip index, wallet index, signature mark.
//!
//! If any write step fails, all prior writes from this mint attempt are reverted so no partial
//! contract state remains. Soroban already rolls back the whole transaction on panic, but this
//! module explicitly undoes intermediate persistent writes when a later step returns `Err`.
//!
//! Signature hashes are only marked used after every storage write succeeds.

use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, String};

use crate::blacklist;
use crate::clip_id_storage;
use crate::creator_portfolio;
use crate::creator_storage;
use crate::mint_authorization;
use crate::mint_event;
use crate::mint_request::BatchMintRequest;
use crate::mint_service;
use crate::mint_validator;
use crate::signature_replay_storage;
use crate::storage_validator;
use crate::token_owner_storage;
use crate::token_storage;
use crate::total_supply;
use crate::types::{BatchMintResponse, DataKey, Error, Royalty, TokenId};
use crate::wallet_token_index;

/// Inputs required to mint a single NFT atomically.
#[derive(Clone)]
#[contracttype]
pub struct MintParams {
    pub owner: Address,
    pub clip_id: u32,
    pub metadata_uri: String,
    pub royalty: Royalty,
    pub signature_hash: BytesN<32>,
    pub creator_address: Option<Address>,
    pub creator_display_name: Option<String>,
}

/// Tracks which persistent keys were written so they can be reverted.
struct MintRollback {
    token_id: TokenId,
    clip_id: u32,
    owner: Address,
    creator_addr: Option<Address>,
    signature_hash: BytesN<32>,
    wrote_owner: bool,
    wrote_metadata: bool,
    wrote_royalty: bool,
    wrote_creator_metadata: bool,
    wrote_creator_portfolio: bool,
    wrote_clip_index: bool,
    wrote_wallet_index: bool,
    wrote_signature: bool,
}

impl MintRollback {
    fn new(
        token_id: TokenId,
        clip_id: u32,
        owner: Address,
        creator_addr: Option<Address>,
        signature_hash: BytesN<32>,
    ) -> Self {
        Self {
            token_id,
            clip_id,
            owner,
            creator_addr,
            signature_hash,
            wrote_owner: false,
            wrote_metadata: false,
            wrote_royalty: false,
            wrote_creator_metadata: false,
            wrote_creator_portfolio: false,
            wrote_clip_index: false,
            wrote_wallet_index: false,
            wrote_signature: false,
        }
    }

    /// Undo every storage write performed during this mint attempt.
    fn revert(&self, env: &Env) {
        if self.wrote_signature {
            signature_replay_storage::unmark_signature_used(env, &self.signature_hash);
        }
        if self.wrote_wallet_index {
            wallet_token_index::remove_token_from_wallet(env, &self.owner, self.token_id);
        }
        if let Some(creator) = &self.creator_addr {
            if self.wrote_creator_portfolio {
                let mut portfolio = creator_portfolio::get_creator_portfolio(env, creator);
                if let Some(pos) = portfolio.iter().position(|t| t == self.token_id) {
                    portfolio.remove(pos as u32);
                    env.storage().persistent().set(
                        &DataKey::CreatorTokens(creator.clone()),
                        &portfolio,
                    );
                }
            }
        }
        if self.wrote_creator_metadata {
            creator_storage::remove_creator_metadata(env, self.token_id);
        }
        if self.wrote_clip_index {
            env.storage()
                .persistent()
                .remove(&DataKey::TokenClipId(self.token_id));
            env.storage()
                .persistent()
                .remove(&DataKey::ClipIdMinted(self.clip_id));
            env.storage()
                .persistent()
                .remove(&DataKey::ClipMinted(self.clip_id));
        }
        if self.wrote_royalty {
            env.storage()
                .persistent()
                .remove(&DataKey::Royalty(self.token_id));
        }
        if self.wrote_metadata {
            env.storage()
                .persistent()
                .remove(&DataKey::Metadata(self.token_id));
        }
        if self.wrote_owner {
            token_owner_storage::remove_owner(env, self.token_id);
        }
    }
}

fn next_token_id(env: &Env) -> Result<TokenId, Error> {
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }
    Ok(env
        .storage()
        .instance()
        .get(&DataKey::NextTokenId)
        .unwrap_or(0))
}

fn commit_token_id(env: &Env, next: TokenId) {
    env.storage()
        .instance()
        .set(&DataKey::NextTokenId, &(next + 1));
}

/// Execute an atomic mint. Returns the new token ID or rolls back on failure.
pub fn execute_atomic_mint(env: &Env, params: &MintParams) -> Result<TokenId, Error> {
    // Phase 1 — read-only validation
    token_owner_storage::validate_owner(env, &params.owner)?;
    signature_replay_storage::ensure_signature_unused(env, &params.signature_hash)?;
    mint_validator::validate_mint(
        env,
        params.clip_id,
        &params.metadata_uri,
        &params.royalty,
        &params.owner,
    )?;
    storage_validator::validate_metadata_uri(&params.metadata_uri)?;
    storage_validator::validate_royalty(env, &params.royalty)?;

    let token_id = next_token_id(env)?;
    let creator_addr = params
        .creator_address
        .clone()
        .unwrap_or_else(|| params.owner.clone());
    let mut rollback = MintRollback::new(
        token_id,
        params.clip_id,
        params.owner.clone(),
        Some(creator_addr.clone()),
        params.signature_hash.clone(),
    );

    // Phase 2 — ordered writes with explicit rollback
    if token_owner_storage::assign_owner(env, token_id, &params.owner, params.clip_id).is_err() {
        rollback.revert(env);
        return Err(Error::InvalidAddress);
    }
    rollback.wrote_owner = true;

    if token_storage::set_metadata(env, token_id, &params.metadata_uri).is_err() {
        rollback.revert(env);
        return Err(Error::InvalidURI);
    }
    rollback.wrote_metadata = true;

    token_storage::set_royalty(env, token_id, &params.royalty);
    rollback.wrote_royalty = true;

    // Write creator metadata
    creator_storage::set_creator_with_name(
        env,
        token_id,
        &creator_addr,
        params.creator_display_name.clone(),
    );
    rollback.wrote_creator_metadata = true;

    // Add to creator portfolio (non-fatal on duplicate, shouldn't happen for new token)
    let _ = creator_portfolio::add_token_to_creator(env, &creator_addr, token_id);
    rollback.wrote_creator_portfolio = true;

    if clip_id_storage::save_clip_id(env, token_id, params.clip_id).is_err() {
        rollback.revert(env);
        return Err(Error::ClipAlreadyMinted);
    }
    rollback.wrote_clip_index = true;

    if wallet_token_index::add_token_to_wallet(env, &params.owner, token_id).is_err() {
        rollback.revert(env);
        return Err(Error::DuplicateWalletEntry);
    }
    rollback.wrote_wallet_index = true;

    // Signature was already verified unused in Phase 1; write without re-reading.
    signature_replay_storage::mark_signature_used_unchecked(env, &params.signature_hash);
    rollback.wrote_signature = true;

    if let Err(e) = total_supply::increment_total_supply(env) {
        rollback.revert(env);
        return Err(e);
    }

    commit_token_id(env, token_id);

    // Legacy lightweight event (backward-compat for existing indexers).
    mint_event::emit_mint(
        env,
        &params.owner,
        params.clip_id,
        token_id,
        &params.metadata_uri,
    );

    // Rich NFTMinted event — emitted only after ALL writes succeed so
    // recipients are guaranteed the token is fully persisted on-chain.
    let timestamp = env.ledger().timestamp();
    mint_event::emit_nft_minted(
        env,
        token_id,
        params.clip_id,
        &creator_addr,
        &params.owner,
        &params.metadata_uri,
        timestamp,
    );

    Ok(token_id)
}

/// Thin contract wrapper used by integration tests.

#[contract]
pub struct AtomicMintContract;

#[contractimpl]
impl AtomicMintContract {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &0u32);
        env.storage()
            .instance()
            .set(&DataKey::NextBatchId, &crate::storage_constants::DEFAULT_NEXT_BATCH_ID);
    }

    pub fn mint(env: Env, params: MintParams) -> Result<TokenId, Error> {
        // #701: Contract must be initialized before any mint is allowed.
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }

        // #699: Caller must be the contract admin or an approved minter.
        mint_authorization::require_mint_auth(&env, &params.owner)?;

        // #700: Blacklisted addresses are blocked from minting.
        if blacklist::is_blacklisted(&env, &params.owner) {
            return Err(Error::Unauthorized);
        }

        execute_atomic_mint(&env, &params)
    }

    pub fn owner_of(env: Env, token_id: TokenId) -> Result<Address, Error> {
        token_owner_storage::get_owner(&env, token_id)
    }

    pub fn tokens_of_owner(env: Env, wallet: Address) -> soroban_sdk::Vec<TokenId> {
        wallet_token_index::get_wallet_tokens(&env, &wallet)
    }

    pub fn signature_used(env: Env, signature_hash: BytesN<32>) -> bool {
        signature_replay_storage::is_signature_used(&env, &signature_hash)
    }

    pub fn token_exists(env: Env, token_id: TokenId) -> bool {
        token_storage::token_exists(&env, token_id)
    }

    pub fn clip_mapped(env: Env, clip_id: u32) -> bool {
        clip_id_storage::is_clip_mapped(&env, clip_id)
    }

    pub fn next_token_id(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0)
    }

    pub fn creator_of(env: Env, token_id: TokenId) -> Result<Address, Error> {
        creator_storage::get_creator(&env, token_id)
    }

    pub fn creator_verified(env: Env, token_id: TokenId) -> Result<bool, Error> {
        creator_storage::is_creator_verified(&env, token_id)
    }

    // ── Issue #701: Contract initialization guard ─────────────────────────────

    /// Returns `true` if the contract has been initialized (i.e. `init` has
    /// been called and the admin address is present in storage).
    ///
    /// Mint operations check this internally; this function lets off-chain
    /// clients verify state before attempting a mint.
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&DataKey::Admin)
    }

    // ── Issue #699: Minter authorization management ───────────────────────────

    /// Grant `minter` the approved-minter role so they may call `mint` and
    /// `batch_mint`.  Caller must be the contract admin.
    pub fn set_approved_minter(env: Env, admin: Address, minter: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        mint_authorization::set_approved_minter(&env, &minter);
        Ok(())
    }

    /// Revoke the approved-minter role from `minter`.  Caller must be the
    /// contract admin.
    pub fn remove_approved_minter(env: Env, admin: Address, minter: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        mint_authorization::remove_approved_minter(&env, &minter);
        Ok(())
    }

    /// Returns `true` if `minter` holds the approved-minter role.
    pub fn is_approved_minter(env: Env, minter: Address) -> bool {
        mint_authorization::is_minter(&env, &minter)
    }

    // ── Issue #700: Blacklist management ──────────────────────────────────────

    /// Add `wallet` to the blacklist, permanently blocking it from minting.
    /// Caller must be the contract admin.
    pub fn add_to_blacklist(env: Env, admin: Address, wallet: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        blacklist::add_wallet(&env, &wallet);
        Ok(())
    }

    /// Remove `wallet` from the blacklist, re-enabling its minting rights.
    /// Caller must be the contract admin.
    pub fn remove_from_blacklist(env: Env, admin: Address, wallet: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        blacklist::remove_wallet(&env, &wallet);
        Ok(())
    }

    /// Returns `true` if `wallet` is currently blacklisted.
    pub fn is_blacklisted(env: Env, wallet: Address) -> bool {
        blacklist::is_blacklisted(&env, &wallet)
    }

    // ── Issue #702: Batch mint ─────────────────────────────────────────────────

    /// Mint multiple NFTs in a single atomic transaction.
    ///
    /// All requests in `batch` are pre-validated before any state is written.
    /// If any single item fails, the entire batch is rolled back — no partial
    /// mints occur.
    ///
    /// # Authorization
    /// `caller` must be the contract admin or an approved minter, and must
    /// not be blacklisted.  The contract must also be initialized.
    ///
    /// # Accepts batch request
    /// `batch` is a [`BatchMintRequest`] containing one or more [`MintRequest`]
    /// items (up to the configured `max_batch_mint_size`).
    ///
    /// # Processes sequentially
    /// Items are minted in order; each token ID increments from the last.
    ///
    /// # Returns batch response
    /// A [`BatchMintResponse`] containing the monotonic `batch_id`, the list of
    /// `minted_token_ids`, `success_count`, `failure_count` (always 0 in the
    /// atomic all-or-nothing mode), and `processed_at` ledger timestamp.
    ///
    /// # Emits batch event
    /// Each individual mint in the batch emits a `"mint"` event; the caller
    /// can correlate them via the returned `batch_id`.
    pub fn batch_mint(
        env: Env,
        caller: Address,
        batch: BatchMintRequest,
    ) -> Result<BatchMintResponse, Error> {
        // #701: Contract must be initialized.
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }

        // #699: Authorization guard — admin or approved minter.
        mint_authorization::require_mint_auth(&env, &caller)?;

        // #700: Blacklist check on the batch caller.
        if blacklist::is_blacklisted(&env, &caller) {
            return Err(Error::Unauthorized);
        }

        // Delegate to the existing batch-mint service which pre-validates
        // every request before writing any state.
        mint_service::execute_batch_mint(&env, &batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature_replay_storage::hash_signature;
    use soroban_sdk::{
        testutils::{Address as _, BytesN as _},
        Address, Env, String,
    };

    fn setup() -> (Env, Address, AtomicMintContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(AtomicMintContract, ());
        let client = AtomicMintContractClient::new(&env, &contract_id);
        client.init(&admin);
        (env, contract_id, client)
    }

    fn sample_params(env: &Env, owner: &Address, clip_id: u32, sig: &BytesN<64>) -> MintParams {
        MintParams {
            owner: owner.clone(),
            clip_id,
            metadata_uri: String::from_str(env, "ipfs://QmTest"),
            royalty: Royalty {
                recipient: owner.clone(),
                basis_points: 500,
                asset_address: None,
            },
            signature_hash: hash_signature(env, sig),
            creator_address: None,
            creator_display_name: None,
        }
    }

    #[test]
    fn successful_mint_assigns_owner_and_indexes_wallet() {
        let (env, _contract, client) = setup();
        let owner = Address::generate(&env);
        let sig = BytesN::<64>::random(&env);
        let params = sample_params(&env, &owner, 1, &sig);

        let token_id = client.mint(&params);
        assert_eq!(token_id, 0);
        assert_eq!(client.owner_of(&token_id), owner);
        assert_eq!(client.tokens_of_owner(&owner).len(), 1);
        assert!(client.signature_used(&params.signature_hash));
        assert_eq!(client.next_token_id(), 1);
    }

    #[test]
    fn replayed_signature_fails_without_state_change() {
        let (env, _contract, client) = setup();
        let owner = Address::generate(&env);
        let sig = BytesN::<64>::random(&env);
        let params = sample_params(&env, &owner, 1, &sig);

        client.mint(&params);
        let result = client.try_mint(&params);
        assert!(result.is_err());
        assert_eq!(client.next_token_id(), 1);
    }

    #[test]
    fn duplicate_clip_rolls_back_partial_writes() {
        let (env, _contract, client) = setup();
        let owner = Address::generate(&env);

        let sig1 = BytesN::<64>::random(&env);
        let params1 = sample_params(&env, &owner, 99, &sig1);
        client.mint(&params1);
        assert!(client.token_exists(&0));

        let sig2 = BytesN::<64>::random(&env);
        let mut params2 = sample_params(&env, &owner, 99, &sig2);
        params2.metadata_uri = String::from_str(&env, "ipfs://QmOther");

        let result = client.try_mint(&params2);
        assert!(result.is_err());
        assert_eq!(client.next_token_id(), 1);
        assert!(client.token_exists(&0));
        assert!(!client.token_exists(&1));
        assert_eq!(client.owner_of(&0), owner);
    }

    #[test]
    fn wallet_index_conflict_rolls_back_prior_writes() {
        let (env, contract_id, client) = setup();
        let owner = Address::generate(&env);

        env.as_contract(&contract_id, || {
            wallet_token_index::add_token_to_wallet(&env, &owner, 0).unwrap();
        });

        let sig = BytesN::<64>::random(&env);
        let params = sample_params(&env, &owner, 77, &sig);
        let result = client.try_mint(&params);
        assert!(result.is_err());
        assert!(!client.token_exists(&0));
        assert_eq!(client.next_token_id(), 0);
        assert!(!client.signature_used(&params.signature_hash));
    }

    #[test]
    fn multiple_mints_increment_wallet_index() {
        let (env, _contract, client) = setup();
        let owner = Address::generate(&env);

        let sig1 = BytesN::<64>::random(&env);
        client.mint(&sample_params(&env, &owner, 1, &sig1));
        let sig2 = BytesN::<64>::random(&env);
        client.mint(&sample_params(&env, &owner, 2, &sig2));

        assert_eq!(client.tokens_of_owner(&owner).len(), 2);
        assert_eq!(client.next_token_id(), 2);
    }

    #[test]
    fn atomic_mint_stores_creator_defaults_to_owner() {
        let (env, _contract, client) = setup();
        let owner = Address::generate(&env);
        let sig = BytesN::<64>::random(&env);
        let params = sample_params(&env, &owner, 10, &sig);

        let token_id = client.mint(&params);

        assert_eq!(client.creator_of(&token_id), owner);
        assert!(!client.creator_verified(&token_id));
    }

    #[test]
    fn atomic_mint_uses_explicit_creator_when_provided() {
        let (env, contract_id, client) = setup();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);
        let sig = BytesN::<64>::random(&env);
        let mut params = sample_params(&env, &owner, 11, &sig);
        params.creator_address = Some(creator.clone());
        params.creator_display_name = Some(String::from_str(&env, "CreatorX"));

        let token_id = client.mint(&params);

        let stored_creator = client.creator_of(&token_id);
        assert_eq!(stored_creator, creator);
        assert_ne!(stored_creator, owner);
        assert!(!client.creator_verified(&token_id));

        env.as_contract(&contract_id, || {
            let dn = creator_storage::get_creator_display_name(&env, token_id).unwrap();
            assert_eq!(dn, Some(String::from_str(&env, "CreatorX")));
        });
    }

    #[test]
    fn duplicate_clip_rolls_back_creator_metadata() {
        let (env, contract_id, client) = setup();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);

        let sig1 = BytesN::<64>::random(&env);
        let mut params1 = sample_params(&env, &owner, 55, &sig1);
        params1.creator_address = Some(creator.clone());
        client.mint(&params1);

        env.as_contract(&contract_id, || {
            assert!(creator_storage::creator_metadata_exists(&env, 0));
        });

        let sig2 = BytesN::<64>::random(&env);
        let params2 = sample_params(&env, &owner, 55, &sig2);
        let result = client.try_mint(&params2);
        assert!(result.is_err());

        env.as_contract(&contract_id, || {
            assert!(!creator_storage::creator_metadata_exists(&env, 1));
            let portfolio = creator_portfolio::get_creator_portfolio(&env, &creator);
            assert_eq!(portfolio.len(), 1);
        });
    }
}
