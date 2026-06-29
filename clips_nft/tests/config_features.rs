#![cfg(test)]

use clips_nft::{
    ClipsNftContract, ClipsNftContractClient, Config, Error,
    MAX_COLLECTION_LIMIT,
};
use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, ClipsNftContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.init(&admin);
    (env, client, admin)
}

// ═══════════════════════════════════════════════════════════════════════════════
// #486 — Configuration Error Types
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_invalid_fee_error_on_high_platform_fee() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 2_000, // exceeds MAX_PLATFORM_FEE_BPS (1_000)
        default_royalty_bps: 500,
        paused: false,
    };
    // set_config should fail with InvalidFee via the validator
    let result = client.try_set_config(&admin, &cfg);
    assert!(result.is_err());
}

#[test]
fn test_invalid_basis_points_error_on_high_royalty() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 100,
        default_royalty_bps: 15_000, // exceeds MAX_ROYALTY_BPS (10_000)
        paused: false,
    };
    let result = client.try_set_config(&admin, &cfg);
    assert!(result.is_err());
}

#[test]
fn test_invalid_limit_error_on_zero_version() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 0, // must be > 0
        platform_fee_bps: 100,
        default_royalty_bps: 500,
        paused: false,
    };
    let result = client.try_set_config(&admin, &cfg);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// #485 — Configuration Update Guard
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_config_guard_allows_admin() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 100,
        default_royalty_bps: 500,
        paused: false,
    };
    // Admin should succeed
    client.set_config(&admin, &cfg);
    let stored = client.get_config();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().platform_fee_bps, 100);
}

#[test]
fn test_config_guard_rejects_non_admin() {
    let (env, client, admin) = setup();
    let non_admin = Address::generate(&env);
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 100,
        default_royalty_bps: 500,
        paused: false,
    };
    // Non-admin should fail with UnauthorizedConfigurationUpdate
    let result = client.try_set_config(&non_admin, &cfg);
    assert!(result.is_err());
}

#[test]
fn test_config_guard_on_set_default_royalty() {
    let (env, client, admin) = setup();
    // Admin should succeed
    client.set_default_royalty_bps(&admin, &500);
    assert_eq!(client.get_default_royalty_bps(), 500);

    // Non-admin should fail
    let non_admin = Address::generate(&env);
    let result = client.try_set_default_royalty_bps(&non_admin, &500);
    assert!(result.is_err());
}

#[test]
fn test_config_guard_on_set_platform_fee() {
    let (env, client, admin) = setup();
    // Admin should succeed
    client.set_platform_fee(&admin, &200);
    assert_eq!(client.get_platform_fee(), 200);

    // Non-admin should fail
    let non_admin = Address::generate(&env);
    let result = client.try_set_platform_fee(&non_admin, &200);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// #483 — Configuration Validator
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_validator_accepts_valid_config() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 500,
        default_royalty_bps: 1_000,
        paused: false,
    };
    client.set_config(&admin, &cfg);
    let stored = client.get_config().unwrap();
    assert_eq!(stored.platform_fee_bps, 500);
    assert_eq!(stored.default_royalty_bps, 1_000);
}

#[test]
fn test_validator_rejects_platform_fee_above_max() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 1_001, // MAX is 1_000
        default_royalty_bps: 500,
        paused: false,
    };
    let result = client.try_set_config(&admin, &cfg);
    assert!(result.is_err());
}

#[test]
fn test_validator_rejects_royalty_above_max() {
    let (env, client, admin) = setup();
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 100,
        default_royalty_bps: 10_001, // MAX is 10_000
        paused: false,
    };
    let result = client.try_set_config(&admin, &cfg);
    assert!(result.is_err());
}

#[test]
fn test_validator_boundary_values_accepted() {
    let (env, client, admin) = setup();
    // Exactly at max platform fee and max royalty
    let cfg = Config {
        owner: admin.clone(),
        version: 1,
        platform_fee_bps: 1_000, // exactly MAX_PLATFORM_FEE_BPS
        default_royalty_bps: 10_000, // exactly MAX_ROYALTY_BPS
        paused: false,
    };
    client.set_config(&admin, &cfg);
    let stored = client.get_config().unwrap();
    assert_eq!(stored.platform_fee_bps, 1_000);
    assert_eq!(stored.default_royalty_bps, 10_000);
}

// ═══════════════════════════════════════════════════════════════════════════════
// #480 — Supported Payment Currency Configuration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_add_currency_success() {
    let (env, client, admin) = setup();
    let currency = Address::generate(&env);
    client.add_currency(&admin, &currency);

    let currencies = client.get_currencies();
    assert_eq!(currencies.len(), 1);
    assert!(client.is_currency_supported(&currency));
}

#[test]
fn test_add_multiple_currencies() {
    let (env, client, admin) = setup();
    let c1 = Address::generate(&env);
    let c2 = Address::generate(&env);
    let c3 = Address::generate(&env);

    client.add_currency(&admin, &c1);
    client.add_currency(&admin, &c2);
    client.add_currency(&admin, &c3);

    let currencies = client.get_currencies();
    assert_eq!(currencies.len(), 3);
    assert!(client.is_currency_supported(&c1));
    assert!(client.is_currency_supported(&c2));
    assert!(client.is_currency_supported(&c3));
}

#[test]
fn test_add_duplicate_currency_fails() {
    let (env, client, admin) = setup();
    let currency = Address::generate(&env);
    client.add_currency(&admin, &currency);

    // Adding same currency again should fail
    let result = client.try_add_currency(&admin, &currency);
    assert!(result.is_err());
}

#[test]
fn test_remove_currency_success() {
    let (env, client, admin) = setup();
    let currency = Address::generate(&env);
    client.add_currency(&admin, &currency);
    assert!(client.is_currency_supported(&currency));

    client.remove_currency(&admin, &currency);
    assert!(!client.is_currency_supported(&currency));
    assert_eq!(client.get_currencies().len(), 0);
}

#[test]
fn test_remove_nonexistent_currency_fails() {
    let (env, client, admin) = setup();
    let currency = Address::generate(&env);

    let result = client.try_remove_currency(&admin, &currency);
    assert!(result.is_err());
}

#[test]
fn test_currency_non_admin_cannot_add() {
    let (env, client, admin) = setup();
    let non_admin = Address::generate(&env);
    let currency = Address::generate(&env);

    let result = client.try_add_currency(&non_admin, &currency);
    assert!(result.is_err());
}

#[test]
fn test_currency_non_admin_cannot_remove() {
    let (env, client, admin) = setup();
    let non_admin = Address::generate(&env);
    let currency = Address::generate(&env);

    client.add_currency(&admin, &currency);
    let result = client.try_remove_currency(&non_admin, &currency);
    assert!(result.is_err());
}

#[test]
fn test_get_currencies_empty_by_default() {
    let (env, client, admin) = setup();
    let currencies = client.get_currencies();
    assert_eq!(currencies.len(), 0);
}

#[test]
fn test_is_currency_supported_false_for_unknown() {
    let (env, client, _admin) = setup();
    let currency = Address::generate(&env);
    assert!(!client.is_currency_supported(&currency));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Treasury wallet (platform recipient)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_treasury_wallet_defaults_to_admin_on_init() {
    let (_env, client, admin) = setup();
    assert_eq!(client.get_platform_recipient(), admin);
}

#[test]
fn test_save_treasury_wallet() {
    let (env, client, admin) = setup();
    let treasury = Address::generate(&env);

    client.save_platform_recipient(&admin, &treasury);
    assert_eq!(client.get_platform_recipient(), treasury);
}

#[test]
fn test_update_treasury_wallet() {
    let (env, client, admin) = setup();
    let treasury = Address::generate(&env);
    let updated = Address::generate(&env);

    client.save_platform_recipient(&admin, &treasury);
    client.update_platform_recipient(&admin, &updated);
    assert_eq!(client.get_platform_recipient(), updated);
}

#[test]
fn test_non_admin_cannot_save_treasury_wallet() {
    let (env, client, _admin) = setup();
    let non_admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    let result = client.try_save_platform_recipient(&non_admin, &treasury);
    assert!(result.is_err());
}

#[test]
fn test_non_admin_cannot_update_treasury_wallet() {
    let (env, client, admin) = setup();
    let non_admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let updated = Address::generate(&env);

    client.save_platform_recipient(&admin, &treasury);
    let result = client.try_update_platform_recipient(&non_admin, &updated);
    assert!(result.is_err());
}
