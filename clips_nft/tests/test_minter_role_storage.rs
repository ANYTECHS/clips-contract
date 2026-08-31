//! Integration tests for the approved minter role storage (issue #698).
//!
//! Exercises `add_approved_minter`, `remove_approved_minter`, and
//! `is_approved_minter` through the public API of the `clips_nft` crate,
//! running inside a real Soroban test environment.

#![cfg(test)]

use clips_nft::{minter_role_storage, AtomicMintContract};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

// ─── tests ────────────────────────────────────────────────────────────────────

/// After adding a minter, `is_approved_minter` must return `true`.
#[test]
fn integration_add_then_query_returns_true() {
    with_contract(|env| {
        let minter = Address::generate(env);
        minter_role_storage::add_approved_minter(env, &minter);
        assert!(
            minter_role_storage::is_approved_minter(env, &minter),
            "minter should be approved after add"
        );
    });
}

/// After removing a minter, `is_approved_minter` must return `false`.
#[test]
fn integration_remove_then_query_returns_false() {
    with_contract(|env| {
        let minter = Address::generate(env);
        minter_role_storage::add_approved_minter(env, &minter);
        minter_role_storage::remove_approved_minter(env, &minter);
        assert!(
            !minter_role_storage::is_approved_minter(env, &minter),
            "minter should not be approved after remove"
        );
    });
}

/// Querying an address that was never added must return `false`.
#[test]
fn integration_query_before_add_returns_false() {
    with_contract(|env| {
        let minter = Address::generate(env);
        assert!(
            !minter_role_storage::is_approved_minter(env, &minter),
            "unapproved address should return false"
        );
    });
}

/// Adding the same minter twice must not panic; status remains `true`.
#[test]
fn integration_add_same_minter_twice_is_idempotent() {
    with_contract(|env| {
        let minter = Address::generate(env);
        minter_role_storage::add_approved_minter(env, &minter);
        minter_role_storage::add_approved_minter(env, &minter);
        assert!(
            minter_role_storage::is_approved_minter(env, &minter),
            "double-add should leave status as true"
        );
    });
}

/// Removing a minter that was never added must not panic.
#[test]
fn integration_remove_non_existent_minter_is_noop() {
    with_contract(|env| {
        let minter = Address::generate(env);
        // Must not panic:
        minter_role_storage::remove_approved_minter(env, &minter);
        assert!(
            !minter_role_storage::is_approved_minter(env, &minter),
            "address should remain unapproved after spurious remove"
        );
    });
}

/// Approvals for different addresses must be stored independently.
#[test]
fn integration_multiple_minters_are_independent() {
    with_contract(|env| {
        let minter_a = Address::generate(env);
        let minter_b = Address::generate(env);
        let minter_c = Address::generate(env);

        minter_role_storage::add_approved_minter(env, &minter_a);
        minter_role_storage::add_approved_minter(env, &minter_b);

        assert!(
            minter_role_storage::is_approved_minter(env, &minter_a),
            "A approved"
        );
        assert!(
            minter_role_storage::is_approved_minter(env, &minter_b),
            "B approved"
        );
        assert!(
            !minter_role_storage::is_approved_minter(env, &minter_c),
            "C not approved"
        );

        // Revoking B must not disturb A or C.
        minter_role_storage::remove_approved_minter(env, &minter_b);

        assert!(
            minter_role_storage::is_approved_minter(env, &minter_a),
            "A still approved"
        );
        assert!(
            !minter_role_storage::is_approved_minter(env, &minter_b),
            "B revoked"
        );
        assert!(
            !minter_role_storage::is_approved_minter(env, &minter_c),
            "C still not approved"
        );
    });
}

/// Approve → revoke → re-approve cycle must leave the minter approved.
#[test]
fn integration_re_approve_after_revoke_works() {
    with_contract(|env| {
        let minter = Address::generate(env);
        minter_role_storage::add_approved_minter(env, &minter);
        minter_role_storage::remove_approved_minter(env, &minter);
        minter_role_storage::add_approved_minter(env, &minter);
        assert!(
            minter_role_storage::is_approved_minter(env, &minter),
            "minter should be approved again after re-add"
        );
    });
}
