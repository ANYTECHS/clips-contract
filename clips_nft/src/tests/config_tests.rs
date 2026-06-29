#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{
    storage::config::{get_config, set_config, validate_config},
    types::Config,
    ClipCashNFT, ClipCashNFTClient,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address, ClipCashNFTClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ClipCashNFT);
    let client = ClipCashNFTClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init(&admin);
    (env, admin, client)
}

fn default_config(env: &Env, admin: &Address) -> Config {
    Config {
        admin: admin.clone(),
        max_royalty_bps: 10_000,
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    }
}

// ── validate_config ───────────────────────────────────────────────────────────

#[test]
fn validate_config_accepts_valid_values() {
    let env = Env::default();
    let admin = Address::generate(&env);
    assert!(validate_config(&default_config(&env, &admin)).is_ok());
}

#[test]
fn validate_config_rejects_max_royalty_over_10000() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let mut cfg = default_config(&env, &admin);
    cfg.max_royalty_bps = 10_001;
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn validate_config_rejects_platform_fee_over_10000() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let mut cfg = default_config(&env, &admin);
    cfg.platform_fee_bps = 10_001;
    assert!(validate_config(&cfg).is_err());
}

#[test]
fn validate_config_accepts_boundary_values() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let cfg = Config {
        admin: admin.clone(),
        max_royalty_bps: 10_000,
        mint_cooldown_secs: u64::MAX,
        platform_fee_bps: 10_000,
    };
    assert!(validate_config(&cfg).is_ok());
}

// ── init & get_config ─────────────────────────────────────────────────────────

#[test]
fn init_sets_default_config() {
    let (env, admin, client) = setup();
    let cfg = client.get_config();
    assert_eq!(cfg.admin, admin);
    assert_eq!(cfg.max_royalty_bps, 10_000);
    assert_eq!(cfg.mint_cooldown_secs, 0);
    assert_eq!(cfg.platform_fee_bps, 0);
}

#[test]
#[should_panic]
fn init_panics_on_reinit() {
    let (_, admin, client) = setup();
    client.init(&admin); // second call should panic
}

// ── set_config ────────────────────────────────────────────────────────────────

#[test]
fn set_config_updates_successfully() {
    let (env, admin, client) = setup();
    let updated = Config {
        admin: admin.clone(),
        max_royalty_bps: 500,
        mint_cooldown_secs: 3600,
        platform_fee_bps: 200,
    };
    client.set_config(&admin, &updated);
    let stored = client.get_config();
    assert_eq!(stored.max_royalty_bps, 500);
    assert_eq!(stored.mint_cooldown_secs, 3600);
    assert_eq!(stored.platform_fee_bps, 200);
}

#[test]
fn set_config_persists_across_calls() {
    let (env, admin, client) = setup();
    let updated = Config {
        admin: admin.clone(),
        max_royalty_bps: 750,
        mint_cooldown_secs: 60,
        platform_fee_bps: 100,
    };
    client.set_config(&admin, &updated);
    // second read returns same values
    let stored = client.get_config();
    assert_eq!(stored.max_royalty_bps, 750);
}

#[test]
fn set_config_rejects_invalid_basis_points() {
    let (env, admin, client) = setup();
    let bad = Config {
        admin: admin.clone(),
        max_royalty_bps: 20_000, // invalid
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    };
    let result = client.try_set_config(&admin, &bad);
    assert!(result.is_err());
}

#[test]
fn set_config_rejects_unauthorized_caller() {
    let (env, _, client) = setup();
    let other = Address::generate(&env);
    let cfg = Config {
        admin: other.clone(),
        max_royalty_bps: 500,
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    };
    let result = client.try_set_config(&other, &cfg);
    assert!(result.is_err());
}

// ── storage persistence (low-level) ──────────────────────────────────────────

#[test]
fn storage_get_set_roundtrip() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, ClipCashNFT);
    env.as_contract(&contract_id, || {
        let cfg = Config {
            admin: admin.clone(),
            max_royalty_bps: 300,
            mint_cooldown_secs: 120,
            platform_fee_bps: 50,
        };
        set_config(&env, &cfg);
        let loaded = get_config(&env);
        assert_eq!(loaded.max_royalty_bps, 300);
        assert_eq!(loaded.mint_cooldown_secs, 120);
        assert_eq!(loaded.platform_fee_bps, 50);
    });
}
