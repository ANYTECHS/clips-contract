//! Integration tests for issues #779 and #781.
//!
//! #779 — RoyaltyRecipient struct: storage, serialization, validation,
//!         and documentation.
//! #781 — Default royalty configuration: store, retrieve, validate, and
//!         contract-level entry points.

#![cfg(test)]

use clips_nft::{
    ClipsNftContract, ClipsNftContractClient, Error, RoyaltyRecipient,
    DEFAULT_ROYALTY_BPS, MAX_ROYALTY_BPS,
};
use clips_nft::royalty_recipient_struct::{new_royalty_recipient, validate_royalty_recipient_struct};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ─── Test setup ───────────────────────────────────────────────────────────────

fn setup() -> (
    &'static Env,
    ClipsNftContractClient<'static>,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let env: &'static Env = Box::leak(Box::new(env));
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.init(&admin);
    (env, client, admin)
}

// ═════════════════════════════════════════════════════════════════════════════
// Issue #779 — RoyaltyRecipient struct
// ═════════════════════════════════════════════════════════════════════════════

// ── Struct fields and Clone ───────────────────────────────────────────────────

/// Verify that `RoyaltyRecipient` fields are publicly accessible.
#[test]
fn royalty_recipient_fields_are_accessible() {
    let env = Env::default();
    let addr = Address::generate(&env);
    let r = RoyaltyRecipient {
        recipient: addr.clone(),
        basis_points: 500,
    };
    assert_eq!(r.recipient, addr);
    assert_eq!(r.basis_points, 500);
}

/// Verify that `RoyaltyRecipient` implements `Clone`.
#[test]
fn royalty_recipient_can_be_cloned() {
    let env = Env::default();
    let addr = Address::generate(&env);
    let original = RoyaltyRecipient {
        recipient: addr.clone(),
        basis_points: 300,
    };
    let cloned = original.clone();
    assert_eq!(cloned.recipient, original.recipient);
    assert_eq!(cloned.basis_points, original.basis_points);
}

// ── Validation ────────────────────────────────────────────────────────────────

/// A valid wallet address and in-range basis points must pass validation.
#[test]
fn royalty_recipient_valid_passes_validation() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let r = RoyaltyRecipient {
            recipient: Address::generate(&env),
            basis_points: 500,
        };
        assert!(validate_royalty_recipient_struct(&env, &r).is_ok());
    });
}

/// Zero basis points must be accepted (creator opts out of royalties).
#[test]
fn royalty_recipient_zero_bps_is_valid() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let r = RoyaltyRecipient {
            recipient: Address::generate(&env),
            basis_points: 0,
        };
        assert!(validate_royalty_recipient_struct(&env, &r).is_ok());
    });
}

/// Maximum basis points (10 000 = 100 %) must be accepted.
#[test]
fn royalty_recipient_max_bps_is_valid() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let r = RoyaltyRecipient {
            recipient: Address::generate(&env),
            basis_points: MAX_ROYALTY_BPS,
        };
        assert!(validate_royalty_recipient_struct(&env, &r).is_ok());
    });
}

/// A value above `MAX_ROYALTY_BPS` must be rejected with `InvalidBasisPoints`.
#[test]
fn royalty_recipient_above_max_bps_returns_invalid_basis_points() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let r = RoyaltyRecipient {
            recipient: Address::generate(&env),
            basis_points: MAX_ROYALTY_BPS + 1,
        };
        assert_eq!(
            validate_royalty_recipient_struct(&env, &r),
            Err(Error::InvalidBasisPoints)
        );
    });
}

/// The contract's own address is not a valid recipient.
#[test]
fn royalty_recipient_contract_address_returns_invalid_recipient() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let r = RoyaltyRecipient {
            recipient: env.current_contract_address(),
            basis_points: 500,
        };
        assert_eq!(
            validate_royalty_recipient_struct(&env, &r),
            Err(Error::InvalidRecipient)
        );
    });
}

// ── Constructor ───────────────────────────────────────────────────────────────

/// `new_royalty_recipient` returns the struct on valid input.
#[test]
fn new_royalty_recipient_succeeds_with_valid_input() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let addr = Address::generate(&env);
        let r = new_royalty_recipient(&env, addr.clone(), 750).unwrap();
        assert_eq!(r.recipient, addr);
        assert_eq!(r.basis_points, 750);
    });
}

/// `new_royalty_recipient` fails with `InvalidBasisPoints` on out-of-range input.
#[test]
fn new_royalty_recipient_rejects_invalid_bps() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let result = new_royalty_recipient(&env, Address::generate(&env), 20_000);
        assert_eq!(result, Err(Error::InvalidBasisPoints));
    });
}

/// `new_royalty_recipient` fails with `InvalidRecipient` for the contract address.
#[test]
fn new_royalty_recipient_rejects_contract_address() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let result = new_royalty_recipient(&env, env.current_contract_address(), 500);
        assert_eq!(result, Err(Error::InvalidRecipient));
    });
}

// ── Two independent recipients ────────────────────────────────────────────────

/// Multiple `RoyaltyRecipient` instances must not share state.
#[test]
fn two_royalty_recipients_are_independent() {
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let ra = new_royalty_recipient(&env, a.clone(), 300).unwrap();
        let rb = new_royalty_recipient(&env, b.clone(), 200).unwrap();
        assert_ne!(ra.recipient, rb.recipient);
        assert_ne!(ra.basis_points, rb.basis_points);
    });
}

// ═════════════════════════════════════════════════════════════════════════════
// Issue #781 — Default royalty configuration
// ═════════════════════════════════════════════════════════════════════════════

// ── Constants exported ────────────────────────────────────────────────────────

/// `DEFAULT_ROYALTY_BPS` should be 500 (5 %) and `MAX_ROYALTY_BPS` 10 000.
#[test]
fn royalty_constants_have_expected_values() {
    assert_eq!(DEFAULT_ROYALTY_BPS, 500);
    assert_eq!(MAX_ROYALTY_BPS, 10_000);
}

// ── Getter default value ──────────────────────────────────────────────────────

/// Before any `set_default_royalty_bps` call the getter returns the constant
/// default (500).
#[test]
fn get_default_royalty_bps_returns_constant_before_any_set() {
    let (_env, client, _admin) = setup();
    assert_eq!(client.get_default_royalty_bps(), DEFAULT_ROYALTY_BPS);
}

// ── Store and retrieve ────────────────────────────────────────────────────────

/// Setting a value and reading it back returns exactly what was stored.
#[test]
fn set_and_get_default_royalty_bps_round_trip() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &750u32);
    assert_eq!(client.get_default_royalty_bps(), 750u32);
}

/// The most recent `set_default_royalty_bps` call overwrites earlier values.
#[test]
fn overwrite_default_royalty_bps_reflects_latest_value() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &100u32);
    client.set_default_royalty_bps(&admin, &9_000u32);
    assert_eq!(client.get_default_royalty_bps(), 9_000u32);
}

// ── Boundary values ───────────────────────────────────────────────────────────

/// 0 bps (0 %) is the lower bound and must be accepted.
#[test]
fn set_default_royalty_bps_zero_accepted() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &0u32);
    assert_eq!(client.get_default_royalty_bps(), 0u32);
}

/// `MAX_ROYALTY_BPS` (10 000 = 100 %) is the upper bound and must be accepted.
#[test]
fn set_default_royalty_bps_max_accepted() {
    let (_env, client, admin) = setup();
    client.set_default_royalty_bps(&admin, &MAX_ROYALTY_BPS);
    assert_eq!(client.get_default_royalty_bps(), MAX_ROYALTY_BPS);
}

// ── Validation ────────────────────────────────────────────────────────────────

/// A value one above `MAX_ROYALTY_BPS` must return `InvalidBasisPoints`.
#[test]
fn set_default_royalty_bps_above_max_returns_invalid_basis_points() {
    let (_env, client, admin) = setup();
    let result = client.try_set_default_royalty_bps(&admin, &10_001u32);
    assert_eq!(result, Err(Ok(Error::InvalidBasisPoints)));
}

/// A clearly out-of-range value must also be rejected.
#[test]
fn set_default_royalty_bps_large_value_rejected() {
    let (_env, client, admin) = setup();
    let result = client.try_set_default_royalty_bps(&admin, &50_000u32);
    assert_eq!(result, Err(Ok(Error::InvalidBasisPoints)));
}

/// `u32::MAX` must be rejected cleanly.
#[test]
fn set_default_royalty_bps_u32_max_rejected() {
    let (_env, client, admin) = setup();
    let result = client.try_set_default_royalty_bps(&admin, &u32::MAX);
    assert_eq!(result, Err(Ok(Error::InvalidBasisPoints)));
}

// ── Authorization ─────────────────────────────────────────────────────────────

/// A non-admin caller must not be able to set the default royalty.
#[test]
fn set_default_royalty_bps_non_admin_rejected() {
    let (env, client, _admin) = setup();
    let non_admin = Address::generate(env);
    let result = client.try_set_default_royalty_bps(&non_admin, &500u32);
    assert_eq!(result, Err(Ok(Error::UnauthorizedConfigurationUpdate)));
}

/// `get_default_royalty_bps` requires no authorization.
#[test]
fn get_default_royalty_bps_requires_no_auth() {
    let env = Env::default();
    // No mock_all_auths — getter should work without any auth.
    let contract_id = env.register(ClipsNftContract, ());
    let env: &'static Env = Box::leak(Box::new(env));
    let client = ClipsNftContractClient::new(env, &contract_id);
    // Uninitialized contract still falls back to the compile-time default.
    assert_eq!(client.get_default_royalty_bps(), DEFAULT_ROYALTY_BPS);
}

// ── Storage isolation ─────────────────────────────────────────────────────────

/// Two separate contract instances must not share default royalty state.
#[test]
fn default_royalty_bps_is_isolated_per_contract_instance() {
    let env = Env::default();
    env.mock_all_auths();
    let env: &'static Env = Box::leak(Box::new(env));

    let admin_a = Address::generate(env);
    let contract_a = env.register(ClipsNftContract, ());
    let client_a = ClipsNftContractClient::new(env, &contract_a);
    client_a.init(&admin_a);

    let admin_b = Address::generate(env);
    let contract_b = env.register(ClipsNftContract, ());
    let client_b = ClipsNftContractClient::new(env, &contract_b);
    client_b.init(&admin_b);

    client_a.set_default_royalty_bps(&admin_a, &1_000u32);
    client_b.set_default_royalty_bps(&admin_b, &2_000u32);

    assert_eq!(client_a.get_default_royalty_bps(), 1_000u32);
    assert_eq!(client_b.get_default_royalty_bps(), 2_000u32);
}
