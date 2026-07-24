#![no_std]

//! `clips_nft` — ClipCash NFT minting modules (Stellar Soroban).

extern crate alloc;

// ─── Core types ───────────────────────────────────────────────────────────────
pub mod types;
pub use types::{
    BurnEvent, CreatorAssignedEvent, DataKey, Error, MintEvent, MetadataUpdatedEvent,
    MintSuccessResponse, RoyaltyInfo, RoyaltyPaidEvent, RoyaltyPayment, Royalty, TokenData,
    TokenId, TransactionStatus, TransferEvent,
};

// ─── Mint pipeline ────────────────────────────────────────────────────────────
pub mod mint_request;
pub use mint_request::{BatchMintRequest, MintRequest};

pub mod mint_service;
pub use mint_service::{execute_mint, execute_mint_with_media, MintResult};

pub mod mint_event;
pub mod mint_validator;
pub mod mint_authorization;

pub mod atomic_mint;
pub use atomic_mint::{AtomicMintContract, AtomicMintContractClient, MintParams};

pub mod signature_replay_storage;
pub use signature_replay_storage::hash_signature;

pub mod total_supply;
pub mod creator_event;
pub mod media_uri_storage;

// ─── Storage ──────────────────────────────────────────────────────────────────
pub mod token_storage;
pub mod token_owner_storage;
pub mod owner_storage;
pub mod royalty_storage;
pub mod clip_id_storage;
pub mod token_uri_storage;
pub mod wallet_token_index;
pub mod minted_clip_index;
pub mod creator_storage;
pub mod storage_validator;
pub mod storage_constants;
pub use storage_constants::{
    CONTRACT_VERSION, CURRENT_MIGRATION_VERSION, DEFAULT_ROYALTY_BPS, DEFAULT_TOTAL_SUPPLY,
    INITIAL_MIGRATION_VERSION, MAX_COLLECTION_LIMIT, MAX_ROYALTY_BPS,
};
pub use storage_constants::CONTRACT_VERSION as VERSION;

// ─── Minting storage tasks ────────────────────────────────────────────────────
pub mod royalty_percentage;
pub mod creator_portfolio;
pub mod owner_portfolio;
pub mod nft_collection;

// ─── Config / royalty helpers used by mint request ────────────────────────────
pub mod config;
pub use config::{Config, ConfigService, MAX_BATCH_MINT_SIZE, MAX_COLLECTION_SIZE};
pub mod platform_fee;
pub mod default_royalty;
pub mod royalty_validator;
