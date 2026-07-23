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
