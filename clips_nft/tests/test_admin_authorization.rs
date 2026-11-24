//! Integration tests for admin authorization (issue #1253).
//!
//! Verifies that [`clips_nft::admin_authorization`] correctly restricts
//! administrative contract operations to the configured administrator.

#![cfg(test)]

use clips_nft::{
    admin_authorization::{get_admin, require_admin_authorization, verify_admin_caller},
    administrator_storage, AtomicMintContract, Error,
};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ─── Test harness ─────────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn setup_admin(env: &Env, admin: &Address) {
    use clips_nft::types::DataKey;
    env.storage().instance().set(&DataKey::Admin, admin);
}

// ─── AC1: Retrieve the configured admin ──────────────────────────────────────

#[test]
fn get_admin_returns_admin_set_during_init() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let retrieved = get_admin(env).expect("admin should be retrievable");
        assert_eq!(retrieved, admin, "retrieved admin must match the stored admin");
    });
}

#[test]
fn get_admin_errors_when_contract_is_uninitialized() {
    with_contract(|env| {
        assert_eq!(
            get_admin(env),
            Err(Error::NotInitialized),
            "should return NotInitialized before any admin is set"
        );
    });
}

// ─── AC2: Verify the caller's identity ───────────────────────────────────────

#[test]
fn verify_admin_caller_returns_true_for_primary_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        assert!(
            verify_admin_caller(env, &admin),
            "primary admin should pass identity verification"
        );
    });
}

#[test]
fn verify_admin_caller_returns_true_for_multi_admin() {
    with_contract(|env| {
        let contract_admin = Address::generate(env);
        let multi_admin = Address::generate(env);
        setup_admin(env, &contract_admin);
        administrator_storage::add_admin(env, &multi_admin);

        assert!(
            verify_admin_caller(env, &multi_admin),
            "registered multi-admin should pass identity verification"
        );
    });
}

#[test]
fn verify_admin_caller_returns_false_for_unauthorized_address() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let unauthorized = Address::generate(env);
        setup_admin(env, &admin);

        assert!(
            !verify_admin_caller(env, &unauthorized),
            "unauthorized address must fail identity verification"
        );
    });
}

#[test]
fn verify_admin_caller_returns_false_when_uninitialized() {
    with_contract(|env| {
        let caller = Address::generate(env);
        assert!(
            !verify_admin_caller(env, &caller),
            "no caller should be verified as admin on an uninitialized contract"
        );
    });
}

// ─── AC3: Reject unauthorized administrative calls ────────────────────────────

#[test]
fn require_admin_authorization_allows_primary_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        assert!(
            require_admin_authorization(env, &admin).is_ok(),
            "primary admin must be authorized"
        );
    });
}

#[test]
fn require_admin_authorization_allows_multi_admin() {
    with_contract(|env| {
        let contract_admin = Address::generate(env);
        let multi_admin = Address::generate(env);
        setup_admin(env, &contract_admin);
        administrator_storage::add_admin(env, &multi_admin);

        assert!(
            require_admin_authorization(env, &multi_admin).is_ok(),
            "registered multi-admin must be authorized"
        );
    });
}

#[test]
fn require_admin_authorization_rejects_unauthorized_caller() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);

        assert_eq!(
            require_admin_authorization(env, &intruder),
            Err(Error::Unauthorized),
            "non-admin caller must be rejected with Unauthorized"
        );
    });
}

#[test]
fn require_admin_authorization_rejects_uninitialized_contract() {
    with_contract(|env| {
        let caller = Address::generate(env);

        assert_eq!(
            require_admin_authorization(env, &caller),
            Err(Error::NotInitialized),
            "calls on an uninitialized contract must return NotInitialized"
        );
    });
}

#[test]
fn require_admin_authorization_not_initialized_takes_priority_over_unauthorized() {
    // Even if a caller would otherwise be unauthorized, the contract
    // initialization check must fire first when no admin is configured.
    with_contract(|env| {
        let caller = Address::generate(env);
        // Register in multi-admin registry, but DataKey::Admin is absent
        administrator_storage::add_admin(env, &caller);

        assert_eq!(
            require_admin_authorization(env, &caller),
            Err(Error::NotInitialized),
            "NotInitialized must be returned before Unauthorized when admin key is absent"
        );
    });
}

#[test]
fn require_admin_authorization_revokes_access_after_multi_admin_removal() {
    with_contract(|env| {
        let contract_admin = Address::generate(env);
        let multi_admin = Address::generate(env);
        setup_admin(env, &contract_admin);
        administrator_storage::add_admin(env, &multi_admin);

        // Granted
        assert!(require_admin_authorization(env, &multi_admin).is_ok());

        // Revoked
        administrator_storage::remove_admin(env, &multi_admin);
        assert_eq!(
            require_admin_authorization(env, &multi_admin),
            Err(Error::Unauthorized),
            "removed multi-admin must be rejected"
        );
    });
}

// ─── AC4: Admin authorization tests — edge cases ──────────────────────────────

#[test]
fn multiple_multi_admins_are_all_authorized() {
    with_contract(|env| {
        let contract_admin = Address::generate(env);
        let multi_admin_1 = Address::generate(env);
        let multi_admin_2 = Address::generate(env);
        let multi_admin_3 = Address::generate(env);
        setup_admin(env, &contract_admin);
        administrator_storage::add_admin(env, &multi_admin_1);
        administrator_storage::add_admin(env, &multi_admin_2);
        administrator_storage::add_admin(env, &multi_admin_3);

        for admin in [&contract_admin, &multi_admin_1, &multi_admin_2, &multi_admin_3] {
            assert!(
                require_admin_authorization(env, admin).is_ok(),
                "each registered admin must be authorized"
            );
        }
    });
}

#[test]
fn authorization_check_is_deterministic_across_repeated_calls() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        for _ in 0..5 {
            assert!(require_admin_authorization(env, &admin).is_ok());
        }
    });
}

#[test]
fn unauthorized_caller_is_consistently_rejected() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);

        for _ in 0..3 {
            assert_eq!(
                require_admin_authorization(env, &intruder),
                Err(Error::Unauthorized)
            );
        }
    });
}

#[test]
fn admin_authorization_composes_with_guard_sequence() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::sequence(
            require_admin_authorization(env, &admin),
            Ok(()), // downstream guard passes
        );
        assert!(result.is_ok(), "composed guard sequence should pass for admin");
    });
}

#[test]
fn unauthorized_failure_propagates_through_guard_sequence() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::sequence(
            require_admin_authorization(env, &intruder),
            Ok(()), // should not execute
        );
        assert_eq!(
            result,
            Err(Error::Unauthorized),
            "unauthorized failure must halt guard composition"
        );
    });
}
