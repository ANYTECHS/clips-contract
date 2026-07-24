#![no_std]

extern crate alloc;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec, String, map, Map};

pub mod blacklist;
pub mod clip_id_storage;
pub mod clip_info_metadata;
pub mod clip_metadata;
pub mod collection_info;
pub mod collection_metadata;
pub mod collection_supply;
pub mod config;
pub mod config_guard;
pub mod config_validator;
pub mod contract_version;
pub mod creator_storage;
pub mod default_royalty;
pub mod errors;
pub mod event_counter_storage;
pub mod frozen_token;
pub mod metadata;
pub mod metadata_config;
pub mod metadata_repository;
pub mod metadata_size;
//! `clips_nft` — ClipCash NFT smart contract (Stellar Soroban).
//!
//! This crate provides the on-chain logic for minting video clips as NFTs
//! with EIP-2981-style royalty support.
//!
//! # Module layout
//! - **types** — shared contract types (`TokenId`, `TokenData`, `Royalty`, …)
//! - **mint_request** — input DTO for mint operations
//! - **mint_service** — orchestrates NFT creation after validation passes
//! - **mint_event** — emits the `"mint"` event after a successful mint
//! - **mint_validator** — pre-mint checks (dedup, URI, blacklist)
//! - **token_storage** — persistent token, metadata, and royalty writes/reads
//! - **clip_id_storage** — bidirectional clip_id ↔ token_id mapping
//! - **minted_clip_index** — clip existence index (dedup sentinel)
//! - **wallet_token_index** — per-owner token list
//! - **pause_guard / pause_state** — circuit-breaker for pausing mint/transfer
//! - **safe_math** — overflow-safe arithmetic helpers

// ─── Core types ───────────────────────────────────────────────────────────────
pub mod types;
pub use types::{
    BurnEvent, DataKey, Error, MintEvent, MetadataUpdatedEvent, RoyaltyInfo,
    RoyaltyPaidEvent, RoyaltyPayment, Royalty, TokenData, TokenId, TransferEvent,
};

// ─── Mint pipeline ────────────────────────────────────────────────────────────
pub mod mint_request;
pub use mint_request::MintRequest;

pub mod mint_service;
pub use mint_service::MintResult;

pub mod mint_event;
pub mod mint_validator;

// ─── Storage modules ──────────────────────────────────────────────────────────
pub mod token_storage;
pub mod owner_storage;
pub mod royalty_storage;
pub mod clip_id_storage;
pub mod token_uri_storage;
pub mod wallet_token_index;
pub mod minted_clip_index;
pub mod creator_storage;
pub mod token_metadata_storage;
pub mod event_counter_storage;
pub mod storage_cleanup;
pub mod storage_validator;
pub mod storage_guard;
pub mod storage_serializer;
pub mod storage_deserializer;
pub use storage_deserializer::{deserialize_metadata, deserialize_royalty, deserialize_token};
pub mod storage;

// ─── Minting storage tasks (issues #673–#676) ─────────────────────────────────
pub mod royalty_percentage;
pub mod creator_portfolio;
pub mod owner_portfolio;
pub mod nft_collection;

// ─── Minting royalty / metadata tasks (issues #666, #667, #670, #671) ─────────
pub mod royalty_recipient_validator;
pub mod mint_royalty_init;
pub mod mint_metadata_link;
pub mod mint_metadata_uri;

// ─── Guard / safety ───────────────────────────────────────────────────────────
pub mod pause_guard;
pub mod pause_state;
pub mod blacklist;
pub mod frozen_token;
pub mod operator_approval;
pub mod token_approval;

// ─── Configuration ────────────────────────────────────────────────────────────
pub mod config;
/// Re-export the `Config` struct from `config` (owner/version/fees model used
/// by the contract ABI).  The legacy `Config` from `types` is accessible via
/// `types::Config`.
pub use config::{Config, ConfigService, MAX_BATCH_MINT_SIZE, MAX_COLLECTION_SIZE};
pub mod config_validator;
pub mod config_guard;
pub mod storage_constants;
pub use storage_constants::{
    CONTRACT_VERSION, CURRENT_MIGRATION_VERSION, DEFAULT_ROYALTY_BPS, DEFAULT_TOTAL_SUPPLY,
    INITIAL_MIGRATION_VERSION, MAX_COLLECTION_LIMIT, MAX_ROYALTY_BPS,
};
/// Alias for [`CONTRACT_VERSION`]; retained for backward compatibility with
/// test code that imports `clips_nft::VERSION`.
pub use storage_constants::CONTRACT_VERSION as VERSION;

// ─── Domain / feature modules ─────────────────────────────────────────────────
pub mod collection_info;
pub mod collection_supply;
pub mod collection_metadata;
pub mod clip_metadata;
pub mod clip_info_metadata;
pub mod metadata;
pub mod metadata_config;
pub mod metadata_repository;
pub mod metadata_timestamps;
pub mod metadata_update_policy;
pub mod metadata_uri_builder;
pub mod metadata_uri_validator;
pub mod metadata_version;
pub mod migration;
pub mod mint_event;
pub mod mint_request;
pub mod mint_validator;
pub mod minted_clip_index;
pub mod operator_approval;
pub mod owner_storage;
pub mod pause_guard;
pub mod pause_state;
pub mod payment_currency;
pub mod platform_fee;
pub mod platform_recipient;
pub mod platform_revenue;
pub mod royalty_config;
pub mod royalty_history;
pub mod royalty_recipient;
pub mod royalty_storage;
pub mod royalty_validator;
pub mod safe_math;
pub mod social_platform;
pub mod storage_cleanup;
pub mod storage_constants;
pub mod storage_deserializer;
pub mod storage_guard;
pub mod storage_serializer;
pub mod storage_validator;
pub mod token_approval;
pub mod token_metadata_storage;
pub mod token_storage;
pub mod token_uri_storage;
pub mod types;
pub mod video_reference;
pub mod virality_score;
pub mod wallet_token_index;

pub mod tests;

pub use mint_request::{MintRequest, BatchMintRequest};
pub mod metadata_size;

pub mod royalty_config;
pub use royalty_config::RoyaltyConfig;
pub mod royalty_recipient;
pub mod royalty_validator;
pub mod royalty_history;
pub mod default_royalty;

pub mod platform_fee;
pub mod platform_recipient;
pub mod platform_revenue;
pub mod payment_currency;
pub mod social_platform;
pub mod video_reference;
pub mod virality_score;

pub mod safe_math;
pub mod contract_version;
pub use contract_version::{get_migration_version, get_upgrade_timestamp, record_upgrade};
pub mod migration;
pub use migration::{is_fully_migrated, migrate_to_current, run_migrations};
pub mod errors;

// ─── Internal unit-test suites ────────────────────────────────────────────────
#[cfg(test)]
mod tests;
#![no_std]

pub mod atomic_mint;
pub mod clip_id_storage;
pub mod mint_event;
pub mod mint_request;
pub mod mint_validator;
pub mod owner_storage;
pub mod signature_replay_storage;
pub mod storage_validator;
pub mod token_owner_storage;
pub mod token_storage;
pub mod types;
pub mod wallet_token_index;

pub use atomic_mint::{AtomicMintContract, AtomicMintContractClient, MintParams};
pub use mint_request::MintRequest;
pub use signature_replay_storage::hash_signature;
pub use types::*;
