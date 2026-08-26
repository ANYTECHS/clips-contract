//! Tests for default royalty configuration.
//!
//! Covers:
//! - Getter returns `DEFAULT_ROYALTY_BPS` (500) before any explicit `set`.
//! - Setter + getter round-trip stores and retrieves the value.
//! - Out-of-range BPS values are rejected with `InvalidBasisPoints`.
//! - `0` and `10_000` (boundary values) are accepted.
//! - Non-admin callers cannot call the setter (auth guard).

#![cfg(test)]

use clips_nft::{ClipsNftContract, Error, DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ─── Helper ───────────────────────────────────────────────────────────────────

/// Register and initialise a fresh contract, returning the client and admin.
fn setup() -> (Env, clips_nft::ClipsNftContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ClipsNftContract, ());
    // SAFETY: the Env is heap-allocated and lives for the lifetime of the test.
    let env: &'static Env = Box::leak(Box::new(env));
    let client = clips_nft::ClipsNftContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.init(&admin);

    (env.clone(), client, admin)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// Before `set_default_royalty_bps` is ever called the getter must return the
/// compile-time default (`DEFAULT_ROYALTY_BPS = 500`).
#[test]
fn test_get_default_royalty_bps_returns_default_before_set() {
    let (_env, client, _admin) = setup();
    assert_eq!(client.get_default_royalty_bps(), DEFAULT_ROYALTY_BPS);
}

/// Setting a value and reading it back must return exactly what was stored.
#[test]
fn test_set_and_get_default_royalty_bps_round_trip() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &750u32);
    assert_eq!(client.get_default_royalty_bps(), 750u32);
}

/// The getter reflects the most recent `set_default_royalty_bps` call.
#[test]
fn test_set_default_royalty_bps_overwrites_previous_value() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &100u32);
    assert_eq!(client.get_default_royalty_bps(), 100u32);

    client.set_default_royalty_bps(&admin, &9_000u32);
    assert_eq!(client.get_default_royalty_bps(), 9_000u32);
}

/// `0` basis points (0 % royalty) must be accepted — a creator may opt out.
#[test]
fn test_set_default_royalty_bps_zero_is_valid() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &0u32);
    assert_eq!(client.get_default_royalty_bps(), 0u32);
}

/// `MAX_ROYALTY_BPS` (10 000 = 100 %) is the highest valid value and must be
/// accepted.
#[test]
fn test_set_default_royalty_bps_max_value_is_valid() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &MAX_ROYALTY_BPS);
    assert_eq!(client.get_default_royalty_bps(), MAX_ROYALTY_BPS);
}

/// Any value strictly above `MAX_ROYALTY_BPS` must be rejected with
/// [`Error::InvalidBasisPoints`].
#[test]
fn test_set_default_royalty_bps_above_max_returns_invalid_basis_points() {
    let (_env, client, admin) = setup();
    let result = client.try_set_default_royalty_bps(&admin, &10_001u32);
    assert_eq!(result, Err(Ok(Error::InvalidBasisPoints)));
}

/// A clearly out-of-range value (e.g. 50 000) must also be rejected.
#[test]
fn test_set_default_royalty_bps_large_value_rejected() {
    let (_env, client, admin) = setup();
    let result = client.try_set_default_royalty_bps(&admin, &50_000u32);
    assert_eq!(result, Err(Ok(Error::InvalidBasisPoints)));
}

/// `u32::MAX` (4 294 967 295) must be rejected cleanly without panicking.
#[test]
fn test_set_default_royalty_bps_u32_max_rejected() {
    let (_env, client, admin) = setup();
    let result = client.try_set_default_royalty_bps(&admin, &u32::MAX);
    assert_eq!(result, Err(Ok(Error::InvalidBasisPoints)));
}

/// A non-admin address must NOT be allowed to call `set_default_royalty_bps`.
///
/// The auth guard (`require_config_admin`) should reject the call with
/// [`Error::UnauthorizedConfigurationUpdate`].
#[test]
fn test_set_default_royalty_bps_non_admin_rejected() {
    let (env, client, _admin) = setup();
    let non_admin = Address::generate(&env);
    let result = client.try_set_default_royalty_bps(&non_admin, &500u32);
    assert_eq!(result, Err(Ok(Error::UnauthorizedConfigurationUpdate)));
}

/// `get_default_royalty_bps` must be readable without any auth.
///
/// This test explicitly does NOT call `mock_all_auths` and still expects the
/// getter to succeed.
#[test]
fn test_get_default_royalty_bps_requires_no_auth() {
    let env = Env::default();
    // No mock_all_auths — getter should work anonymously.
    let contract_id = env.register(ClipsNftContract, ());
    let env: &'static Env = Box::leak(Box::new(env));
    let client = clips_nft::ClipsNftContractClient::new(env, &contract_id);

    // Init the contract with mocked auth just for setup.
    {
        let env2 = Env::default();
        env2.mock_all_auths();
        let contract_id2 = env2.register(ClipsNftContract, ());
        let env2: &'static Env = Box::leak(Box::new(env2));
        let client2 = clips_nft::ClipsNftContractClient::new(env2, &contract_id2);
        let admin = Address::generate(env2);
        client2.init(&admin);
        // Just verify the default is readable.
        assert_eq!(client2.get_default_royalty_bps(), DEFAULT_ROYALTY_BPS);
    }

    // Uninitialized contract: getter still returns the fallback default.
    assert_eq!(client.get_default_royalty_bps(), DEFAULT_ROYALTY_BPS);
}

/// Storage isolation: two separate contract instances must not share default
/// royalty state.
#[test]
fn test_default_royalty_bps_is_isolated_per_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let env: &'static Env = Box::leak(Box::new(env));

    let admin_a = Address::generate(env);
    let admin_b = Address::generate(env);

    let contract_a = env.register(ClipsNftContract, ());
    let client_a = clips_nft::ClipsNftContractClient::new(env, &contract_a);
    client_a.init(&admin_a);

    let contract_b = env.register(ClipsNftContract, ());
    let client_b = clips_nft::ClipsNftContractClient::new(env, &contract_b);
    client_b.init(&admin_b);

    client_a.set_default_royalty_bps(&admin_a, &1_000u32);
    client_b.set_default_royalty_bps(&admin_b, &2_000u32);

    assert_eq!(client_a.get_default_royalty_bps(), 1_000u32);
    assert_eq!(client_b.get_default_royalty_bps(), 2_000u32);
}
