//! `clips_nft` — ClipCash NFT smart contract (Stellar Soroban).
//!
//! This crate provides the on-chain logic for minting video clips as NFTs
//! with EIP-2981-style royalty support.

#![no_std]

extern crate alloc;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Map, String, Vec};

// ─── Core types ───────────────────────────────────────────────────────────────
pub mod types;
pub use types::{
    BurnEvent, DataKey, Error, MetadataUpdatedEvent, MintEvent, Royalty, RoyaltyInfo,
    RoyaltyPaidEvent, RoyaltyPayment, TokenData, TokenId, TransferEvent,
};
pub mod contract_version;
pub mod default_royalty;
pub mod errors;

// ─── Mint pipeline ────────────────────────────────────────────────────────────
pub mod mint_request;
pub use mint_request::{BatchMintRequest, MintRequest};

pub mod mint_service;
pub use mint_service::{execute_mint, execute_mint_with_media, MintResult};

pub mod mint_event;
pub mod mint_validator;
pub mod mint_authorization;

// ─── Storage modules ──────────────────────────────────────────────────────────
pub mod clip_id_storage;
pub mod creator_storage;
pub mod event_counter_storage;
pub mod minted_clip_index;
pub mod owner_storage;
pub mod royalty_storage;
pub mod storage;
pub mod storage_cleanup;
pub mod storage_deserializer;
pub mod storage_guard;
pub mod storage_serializer;
pub mod storage_validator;
pub mod token_metadata_storage;
pub mod token_storage;
pub mod token_uri_storage;
pub mod wallet_token_index;
pub use storage_deserializer::{deserialize_metadata, deserialize_royalty, deserialize_token};

// ─── Minting feature modules (issues #665, #668, #669, #672) ─────────────────
pub mod preview_video_uri;
pub mod thumbnail_uri;

// ─── Minting storage tasks (issues #673–#676) ────────────────────────────────
pub mod creator_portfolio;
pub mod nft_collection;
pub mod owner_portfolio;
pub mod royalty_percentage;

// ─── Minting royalty / metadata tasks (issues #666, #667, #670, #671) ─────────
pub mod royalty_recipient_validator;
pub mod mint_royalty_init;
pub mod mint_metadata_link;
pub mod mint_metadata_uri;

// ─── Guard / safety ───────────────────────────────────────────────────────────
pub mod blacklist;
pub mod frozen_token;
pub mod operator_approval;
pub mod pause_guard;
pub mod pause_state;
pub mod token_approval;

// ─── Configuration ────────────────────────────────────────────────────────────
pub mod config;
pub use config::{Config, ConfigService, MAX_BATCH_MINT_SIZE, MAX_COLLECTION_SIZE};
pub mod config_guard;
pub mod config_validator;
pub mod storage_constants;
pub use storage_constants::{
    CONTRACT_VERSION, CURRENT_MIGRATION_VERSION, DEFAULT_ROYALTY_BPS, DEFAULT_TOTAL_SUPPLY,
    INITIAL_MIGRATION_VERSION, MAX_COLLECTION_LIMIT, MAX_ROYALTY_BPS,
};
/// Alias for [`CONTRACT_VERSION`]; retained for backward compatibility.
pub use storage_constants::CONTRACT_VERSION as VERSION;

// ─── Domain / feature modules ─────────────────────────────────────────────────
pub mod metadata;
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
pub mod migration;
pub use migration::{is_fully_migrated, migrate_to_current, run_migrations};
pub mod payment_currency;
pub mod platform_fee;
pub mod platform_recipient;
pub mod platform_revenue;
pub mod royalty_config;
pub use royalty_config::RoyaltyConfig;
pub mod royalty_history;
pub mod royalty_recipient;
pub mod royalty_validator;
pub mod safe_math;
pub mod social_platform;
pub mod video_reference;
pub mod virality_score;

// ─── Atomic mint executor ─────────────────────────────────────────────────────
pub mod atomic_mint;
pub mod mint_authorization;
pub mod signature_replay_storage;
pub use signature_replay_storage::hash_signature;
pub mod token_id_generator;
pub mod token_owner_storage;

// ─── Internal unit-test suites ────────────────────────────────────────────────
#[cfg(test)]
pub mod tests;
