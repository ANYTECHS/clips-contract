//! Tests for issues #454, #456, and #457.
//!
//! #454 — get_contract_metadata returns all expected fields
//! #456 — mutable public functions return NotInitialized before init()
//! #457 — get_global_config / set_global_config round-trip

#![cfg(test)]

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Error};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ---------------------------------------------------------------------------
// #456 — initialization guard
// ---------------------------------------------------------------------------

/// A contract that has NOT been initialised must reject every mutable call.
#[test]
fn test_require_initialized_before_mint() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    // burn should fail with NotInitialized
    let result = client.try_burn(&user, &1u32);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_require_initialized_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    let from = Address::generate(&env);
    let to = Address::generate(&env);

    let result = client.try_transfer(&from, &to, &1u32, &0i128, &None);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_require_initialized_approve() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    let caller = Address::generate(&env);
    let operator = Address::generate(&env);

    let result = client.try_approve(&caller, &Some(operator), &1u32);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_require_initialized_set_approval_for_all() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    let caller = Address::generate(&env);
    let operator = Address::generate(&env);

    let result = client.try_set_approval_for_all(&caller, &operator, &true);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_no_error_after_init() {
    let ctx = test_helpers::setup();
    // If init succeeded we can at least query total_supply without error.
    assert_eq!(ctx.client.total_supply(), 0u32);
}

// ---------------------------------------------------------------------------
// #454 — get_contract_metadata
// ---------------------------------------------------------------------------

#[test]
fn test_get_contract_metadata_returns_all_fields() {
    let ctx = test_helpers::setup();
    let meta = ctx.client.get_contract_metadata();

    // collection name and symbol
    assert_eq!(meta.collection_name.to_string(), "ClipCash Clips");
    assert_eq!(meta.collection_symbol.to_string(), "CLIP");

    // version from constant
    assert_eq!(meta.version, clips_nft::VERSION);

    // defaults set by init
    assert_eq!(meta.platform_fee_bps, 100u32);
    assert_eq!(meta.default_royalty_bps, 0u32);

    // owner is the admin passed to init
    assert_eq!(meta.owner, ctx.admin);
}

#[test]
fn test_get_contract_metadata_reflects_fee_change() {
    let ctx = test_helpers::setup();
    ctx.client.set_platform_fee(&ctx.admin, &250u32);

    let meta = ctx.client.get_contract_metadata();
    assert_eq!(meta.platform_fee_bps, 250u32);
}

// ---------------------------------------------------------------------------
// #457 — global config storage
// ---------------------------------------------------------------------------

#[test]
fn test_get_global_config_defaults() {
    let ctx = test_helpers::setup();
    let cfg = ctx.client.get_global_config();

    assert_eq!(cfg.platform_fee_bps, 100u32);
    assert_eq!(cfg.default_royalty_bps, 0u32);
    assert_eq!(cfg.mint_cooldown_seconds, 0u64);
    assert!(!cfg.circuit_breaker_enabled);
    assert_eq!(cfg.circuit_breaker_threshold, 100u64);
    assert_eq!(cfg.circuit_breaker_window_seconds, 60u64);
}

#[test]
fn test_set_global_config_round_trip() {
    let ctx = test_helpers::setup();

    let new_cfg = clips_nft::config::GlobalConfig {
        platform_fee_bps: 200,
        default_royalty_bps: 500,
        mint_cooldown_seconds: 3600,
        circuit_breaker_enabled: true,
        circuit_breaker_threshold: 50,
        circuit_breaker_window_seconds: 120,
    };

    ctx.client.set_global_config(&ctx.admin, &new_cfg);

    let read_back = ctx.client.get_global_config();
    assert_eq!(read_back, new_cfg);
}

#[test]
fn test_set_global_config_rejects_invalid_fee() {
    let ctx = test_helpers::setup();

    let bad_cfg = clips_nft::config::GlobalConfig {
        platform_fee_bps: 10_001, // > 100%
        default_royalty_bps: 0,
        mint_cooldown_seconds: 0,
        circuit_breaker_enabled: false,
        circuit_breaker_threshold: 100,
        circuit_breaker_window_seconds: 60,
    };

    let result = ctx.client.try_set_global_config(&ctx.admin, &bad_cfg);
    assert_eq!(result, Err(Ok(Error::RoyaltyTooHigh)));
}

#[test]
fn test_set_global_config_requires_admin() {
    let ctx = test_helpers::setup();
    let non_admin = Address::generate(ctx.env);

    let cfg = ctx.client.get_global_config();
    let result = ctx.client.try_set_global_config(&non_admin, &cfg);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}
