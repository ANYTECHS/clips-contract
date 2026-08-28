//! `clips_nft` — ClipCash NFT smart contract (Stellar Soroban).
//!
//! This crate provides the on-chain logic for minting video clips as NFTs
//! with EIP-2981-style royalty support.

#![no_std]

extern crate alloc;

use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct ClipCashNFT;

#[contractimpl]
impl ClipCashNFT {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&crate::types::DataKey::Config) {
            panic!("already initialized");
        }
        crate::storage::config::set_config(
            &env,
            &crate::types::Config {
                admin: admin.clone(),
                max_royalty_bps: crate::storage_constants::DEFAULT_ROYALTY_BPS,
                mint_cooldown_secs: 0,
                platform_fee_bps: 0,
            },
        );
    }

    pub fn get_config(env: Env) -> crate::types::Config {
        crate::storage::config::get_config(&env)
    }

    pub fn set_config(
        env: Env,
        updater: Address,
        config: crate::types::Config,
    ) -> Result<(), crate::types::Error> {
        let current = crate::storage::config::get_config(&env);
        if current.admin != updater {
            return Err(crate::types::Error::Unauthorized);
        }
        crate::storage::config::validate_config(&config)?;
        crate::storage::config::set_config(&env, &config);
        Ok(())
    }
}

// ─── Core types ───────────────────────────────────────────────────────────────
pub mod types;
pub use types::{
    BatchId, BatchMintResponse, BurnEvent, DataKey, Error, Listing, ListingStatus, MetadataUpdatedEvent,
    MintEvent, MintSuccessResponse, NFTMintedEvent, Royalty, RoyaltyInfo, RoyaltyPaidEvent,
    RoyaltyPayment, RoyaltyPaymentResult, RoyaltyRecipient, RoyaltyUpdatedEvent, TokenData, TokenId,
    TransactionStatus, TransferEvent, TransferResult,
    BatchId, BatchMintResponse, BurnEvent, DataKey, Error, MetadataUpdatedEvent, MintEvent,
    MintSuccessResponse, NFTMintedEvent, Royalty, RoyaltyFrozenEvent, RoyaltyInfo, RoyaltyPaidEvent, RoyaltyPayment,
    RoyaltyPaymentResult, RoyaltyRecipient, TokenData, TokenId, TransactionStatus, TransferEvent,
    TransferResult,
    ListingId, RoyaltyRecipient, TokenData, TokenId, TransactionStatus, TransferEvent, TransferResult,
};
pub mod contract_version;
pub mod default_royalty;
pub mod errors;

// ─── Metadata types ───────────────────────────────────────────────────────────
pub mod metadata;
pub use crate::metadata::{Attribute, ClipMetadata, CreatorMetadata, MetadataImage, TokenMetadata};
pub use metadata::{Attribute, ClipMetadata, CreatorMetadata, MetadataImage, TokenMetadata};

// ─── Mint pipeline ────────────────────────────────────────────────────────────
pub mod mint_request;
pub use mint_request::{BatchMintRequest, MintRequest};

pub mod transfer_request;
pub use transfer_request::{BatchTransferRequest, TransferRequest};

pub mod listing_request;
pub use listing_request::ListingRequest;
pub mod listing_storage;
pub mod listing_id_generator;

pub mod batch_mint_event;
pub mod creator_event;
pub mod mint_event;
pub mod batch_mint_event;
pub mod royalty_assigned_event;
pub mod royalty_frozen_event;
pub mod royalty_updated_event;
pub mod mint_validator;
pub use mint_validator::{validate_batch_mint, validate_mint, validate_mint_request};

/// Mint authorization guard — reusable check for all minting entry-points.
pub mod mint_authorization;
pub use mint_authorization::{
    is_minter, remove_approved_minter, require_mint_auth, set_approved_minter,
};

pub mod mint_service;
pub use mint_service::{execute_batch_mint, execute_mint, execute_mint_with_media, MintResult};

// ─── Storage modules ──────────────────────────────────────────────────────────
pub mod administrator_storage;
pub mod clip_id_storage;
pub mod creator_storage;
pub mod event_counter_storage;
pub mod media_uri_storage;
pub mod metadata_manager;
pub mod minted_clip_index;
pub mod minter_role_storage;
pub mod owner_storage;
pub mod royalty_storage;
pub mod storage;
pub mod storage_cleanup;
pub mod storage_deserializer;
pub mod storage_guard;
pub mod storage_serializer;
pub mod storage_validator;
pub mod token_counter_storage;
pub mod token_metadata_storage;
pub mod token_storage;
pub mod token_uri_storage;
pub mod total_supply;
pub mod verify_mint;
pub mod wallet_token_index;
pub use storage_deserializer::{deserialize_metadata, deserialize_royalty, deserialize_token};

// ─── Minting feature modules (issues #665, #668, #669, #672) ─────────────────
pub mod preview_video_uri;
pub mod thumbnail_uri;

// ─── Minting storage tasks (issues #673–#676) ────────────────────────────────
pub mod creator_portfolio;
pub mod creator_royalty;
pub mod nft_collection;
pub mod owner_portfolio;
pub mod royalty_percentage;

// ─── Minting royalty / metadata tasks (issues #666, #667, #670, #671) ─────────
pub mod mint_metadata_link;
pub mod mint_metadata_uri;
pub mod mint_royalty_init;
pub mod royalty_earnings;
pub mod royalty_payment;
pub mod royalty_payment_replay;
pub mod royalty_recipient_validator;

// ─── Guard / safety ───────────────────────────────────────────────────────────
pub mod blacklist;
pub mod frozen_token;
pub mod operator_approval;
pub mod pause_guard;
pub mod pause_state;
pub mod token_approval;
pub mod transfer_guard;

// ─── Royalty guards (issues #843, #847) ──────────────────────────────────────
pub mod royalty_admin_guard;
pub mod royalty_pause_guard;

// ─── Marketplace (issues #851, #862) ─────────────────────────────────────────
pub mod marketplace;

// ─── Configuration ────────────────────────────────────────────────────────────
pub mod config;
pub use config::{Config, ConfigService, MAX_BATCH_MINT_SIZE, MAX_COLLECTION_SIZE};
pub mod config_guard;
pub mod config_validator;
pub mod storage_constants;
/// Alias for [`CONTRACT_VERSION`]; retained for backward compatibility.
pub use storage_constants::CONTRACT_VERSION as VERSION;
pub use storage_constants::{CONTRACT_VERSION, DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS};

// ─── Domain / feature modules ─────────────────────────────────────────────────
pub mod clip_info_metadata;
pub mod clip_metadata;
pub mod metadata_config;
pub mod metadata_repository;
pub mod metadata_size;
pub mod metadata_timestamps;
pub mod metadata_update_policy;
pub mod metadata_uri_builder;
pub mod metadata_uri_validator;
pub mod metadata_version;
pub use metadata_version::{
    get_metadata_version, get_version, MetadataVersion, DEFAULT_METADATA_VERSION,
};
pub mod migration;
pub use migration::{is_fully_migrated, migrate_to_current, run_migrations};
pub mod net_seller_amount;
pub use net_seller_amount::{calculate_net_seller_amount, NetSellerAmount};
pub mod payment_currency;
pub mod platform_fee;
pub mod platform_recipient;
pub mod platform_revenue;
pub mod royalty_config;
pub use royalty_config::RoyaltyConfig;
pub mod royalty_recipient_validation;
pub use royalty_recipient_validation::{
    validate_royalty_recipient_address, validate_royalty_recipient as validate_recipient,
};
pub mod maximum_royalty;
pub use maximum_royalty::{
    allowed_royalty_bps, get_max_royalty_bps, has_max_royalty_bps, set_max_royalty_bps,
    validate_royalty_within_max,
};
pub mod nft_royalty_storage;
pub use nft_royalty_storage::{
    get_nft_royalty_config, has_nft_royalty_config, set_nft_royalty_config,
};
pub mod royalty_percentage_validator;
pub use royalty_percentage_validator::validate_royalty_percentage;
pub mod royalty_history;
pub mod royalty_recipient;
pub mod royalty_recipient_index;
pub mod royalty_recipient_struct;
pub use royalty_recipient_struct::{new_royalty_recipient, validate_royalty_recipient_struct};
pub mod royalty_authorization;
pub mod royalty_validator;
pub mod safe_math;
pub use royalty_authorization::authorize_royalty_update;
pub mod royalty_freeze;
pub use royalty_freeze::{freeze_royalty, is_royalty_frozen};
pub mod royalty_lifecycle;
pub use royalty_lifecycle::{royalty_state, validate_state_for_update, RoyaltyLifecycleState};
pub mod royalty_updater;
pub use royalty_updater::update_royalty_configuration;
pub mod royalty_calculation;
pub use royalty_calculation::{basis_point_percentage, calculate_royalty_amount, is_zero_royalty};
pub mod royalty_validation_pipeline;
pub use royalty_validation_pipeline::{
    authorize_royalty_update, is_royalty_frozen, validate_royalty_configuration,
    validate_royalty_operation, validate_royalty_state, validate_token_exists,
};
pub mod social_platform;
pub mod video_reference;
pub mod virality_score;

// ─── Transaction deduction validator (issue #807) ────────────────────────────
pub mod transaction_deduction_validator;

// ─── Royalty asset validator (issue #810) ───────────────────────────────────
pub mod royalty_asset_validator;

// ─── Atomic mint executor ─────────────────────────────────────────────────────
pub mod atomic_mint;
pub use atomic_mint::AtomicMintContract;

pub mod batch_id_storage;
pub mod signature_replay_storage;
pub use signature_replay_storage::hash_signature;
pub mod token_id_generator;
pub mod token_owner_storage;

// ─── ClipsNftContract — primary on-chain contract ─────────────────────────────
//
// This is the main deployable contract that exposes all public entry points.
// Storage helper functions live in their respective modules; this impl block
// wires them to the Soroban ABI.

/// The primary ClipCash NFT contract.
///
/// Exposes all public entry points:
/// - Contract initialization (`init`)
/// - Default royalty configuration (`set_default_royalty_bps`, `get_default_royalty_bps`)
///
/// Storage delegation pattern: each entry point validates caller auth, then
/// delegates to the appropriate storage module.
#[contract]
pub struct ClipsNftContract;

#[contractimpl]
impl ClipsNftContract {
    // ── Initialization ────────────────────────────────────────────────────────

    /// Initialize the contract, recording `admin` as the sole administrator.
    ///
    /// Must be called exactly once before any other entry point. Subsequent
    /// calls panic with "already initialized".
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &0u32);
        env.storage().instance().set(
            &DataKey::NextBatchId,
            &crate::storage_constants::DEFAULT_NEXT_BATCH_ID,
        );
    }

    // ── Default royalty configuration (issues #486, #485, #483) ─────────────

    /// Store the contract-wide default royalty basis points.
    ///
    /// This value is applied to newly minted NFTs when no per-token royalty
    /// is explicitly provided.
    ///
    /// # Authorization
    /// Caller must be the contract admin.
    ///
    /// # Validation
    /// `bps` must be in `0..=10_000` (0 %–100 %). Returns
    /// [`Error::InvalidBasisPoints`] for out-of-range values.
    ///
    /// # Storage
    /// Written to `DataKey::DefaultRoyaltyBps` in instance storage.
    pub fn set_default_royalty_bps(env: Env, admin: Address, bps: u32) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        default_royalty::set_default_royalty_bps(&env, bps)
    }

    /// Return the current contract-wide default royalty in basis points.
    pub fn get_default_royalty_bps(env: Env) -> u32 {
        default_royalty::get_default_royalty_bps(&env)
    }

    /// Process a royalty payment for a secondary sale (issues #809, #810, #831, #832, #833, #837).
    ///
    /// Computes the royalty amount from `sale_price` using the token's configured
    /// royalty, validates the recipient and asset, executes the transfer, records
    /// the payment, increments cumulative earnings, and emits a `RoyaltyPaidEvent`.
    ///
    /// Replay protection is enforced: re-processing the same payment returns
    /// [`Error::PaymentAlreadyProcessed`].
    pub fn pay_royalty(env: Env, payer: Address, token_id: TokenId, sale_price: i128) -> Result<(), Error> {
    /// Retrieve the cumulative royalty earnings generated by a token.
    pub fn get_cumulative_earnings(env: Env, token_id: u32) -> i128 {
        royalty_earnings::get_cumulative_earnings(&env, token_id)
    }

    // ── Royalty payment (issues #809, #810) ─────────────────────────────────
    // ─── Royalty payment (issues #809, #810) ─────────────────────────────────

    /// Execute a royalty payment for a token secondary sale.
    pub fn pay_royalty(
        env: Env,
        payer: Address,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<RoyaltyPaymentResult, Error> {
        royalty_payment::pay_royalty(&env, &payer, token_id, sale_price)
    }

    /// Return royalty info for a token (read-only preview).
    pub fn royalty_info(
        env: Env,
        token_id: TokenId,
        sale_price: i128,
    ) -> Result<RoyaltyInfo, Error> {
        royalty_payment::royalty_info(&env, token_id, sale_price)
    }

    /// Set royalty configuration for a token (issues #791, #831).
    ///
    /// Verifies the token exists before assignment, returning
    /// [`Error::TokenNotFound`] for nonexistent NFTs.
    /// Retrieve the cumulative royalty earnings generated by a token.
    pub fn get_cumulative_earnings(env: Env, token_id: TokenId) -> i128 {
        royalty_earnings::get_cumulative_earnings(&env, token_id)
    }

    /// Set royalty configuration for a token.
    pub fn set_royalty(
        env: Env,
        admin: Address,
        token_id: TokenId,
        royalty: Royalty,
    ) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &admin)?;
        if env
            .storage()
            .persistent()
            .has(&DataKey::RoyaltyFrozen(token_id))
        {
            return Err(Error::RoyaltyFrozen);
        }
        royalty_validator::validate_royalty(&royalty)?;
        crate::royalty_recipient_validator::validate_royalty_recipients(&env, &royalty)?;
        token_storage::require_token_exists(&env, token_id)?;
        token_storage::set_royalty(&env, token_id, &royalty);
        Ok(())
    }

    /// Permanently freeze royalty configuration for a token.
    pub fn freeze_royalty(env: Env, caller: Address, token_id: TokenId) -> Result<(), Error> {
        config_guard::require_config_admin(&env, &caller)?;
        royalty_storage::freeze_royalty(&env, token_id)?;
        royalty_frozen_event::emit_royalty_frozen(&env, token_id, &caller, env.ledger().timestamp());
        Ok(())
    }

    /// Get royalty configuration for a token.
    pub fn get_royalty(env: Env, token_id: TokenId) -> Result<Royalty, Error> {
        token_storage::get_royalty(&env, token_id)
    }

    pub fn create_listing(env: Env, listing: ListingRequest) -> Result<ListingId, Error> {
        listing.seller.require_auth();
        listing_storage::create_listing(&env, &listing)
    }

    pub fn get_listing(env: Env, token_id: TokenId) -> Result<ListingRequest, Error> {
        listing_storage::get_listing(&env, token_id)
    }

    pub fn cancel_listing(env: Env, seller: Address, token_id: TokenId) -> Result<(), Error> {
        seller.require_auth();
        let listing = listing_storage::get_listing(&env, token_id)?;
        if listing.seller != seller {
            return Err(Error::Unauthorized);
        }
        listing_storage::remove_listing(&env, token_id)
    }

    pub fn complete_listing(env: Env, buyer: Address, token_id: TokenId) -> Result<(), Error> {
        buyer.require_auth();
        listing_storage::remove_listing(&env, token_id)
    /// Retrieve the cumulative royalty earnings generated by a token (issue #835).
    pub fn get_cumulative_earnings(env: Env, token_id: TokenId) -> i128 {
        royalty_earnings::get_cumulative_earnings(&env, token_id)
    }

    /// Retrieve the cumulative royalty earnings for a creator (issue #834).
    pub fn get_creator_earnings(env: Env, creator: Address) -> i128 {
        royalty_earnings::get_creator_earnings(&env, &creator)
    }

    /// Return the royalty payment history recorded for a token (issue #833).
    pub fn get_royalty_history(env: Env, token_id: TokenId) -> soroban_sdk::Vec<RoyaltyPayment> {
        royalty_history::get_royalty_history(&env, token_id)
    // ── Royalty lifecycle control (issues #794, #795) ──────────────────────

    /// Permanently freeze a token's royalty configuration (issue #794).
    ///
    /// Once frozen, the configuration can never be modified. Restricted to
    /// the contract admin, the token creator, or the token owner.
    pub fn freeze_royalty(env: Env, caller: Address, token_id: TokenId) -> Result<(), Error> {
        royalty_freeze::freeze_royalty(&env, &caller, token_id)
    }

    /// Return whether a token's royalty configuration is frozen (issue #794).
    pub fn is_royalty_frozen(env: Env, token_id: TokenId) -> bool {
        royalty_freeze::is_royalty_frozen(&env, token_id)
    }

    /// Update a token's royalty configuration (issue #793).
    ///
    /// Restricted to the contract admin, the token creator, or the token
    /// owner. Rejected for unknown tokens, frozen configurations, and invalid
    /// incoming values.
    pub fn update_royalty(
        env: Env,
        caller: Address,
        token_id: TokenId,
        royalty: Royalty,
    ) -> Result<(), Error> {
        royalty_updater::update_royalty_configuration(&env, &caller, token_id, &royalty)
    }
}

// ─── Internal unit-test suites ────────────────────────────────────────────────
#[cfg(test)]
pub mod tests;
