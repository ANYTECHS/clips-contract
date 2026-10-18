//! Integration tests for the Creator Authorization Guard (issue #1249).
//!
//! These tests exercise [`clips_nft::creator_guard`] and
//! [`clips_nft::creator_auth_storage`] through a fully registered Soroban
//! contract environment to ensure the guard integrates correctly with live
//! persistent storage.
//!
//! # Coverage
//!
//! ## `require_creator` (creator-only)
//! - Registered creator passes the guard.
//! - Unrelated caller is rejected.
//! - Authorized delegate is rejected (creator-only path).
//! - Missing token record returns `TokenNotFound`.
//!
//! ## `require_creator_or_authorized` (creator OR delegate)
//! - Registered creator passes.
//! - Authorized delegate passes.
//! - Unrelated third party is rejected.
//! - Missing token record returns `TokenNotFound`.
//! - Revoked delegate is rejected.
//! - Replaced delegate: old rejected, new accepted.
//! - Authorization is isolated per token.
//!
//! ## `is_creator_or_authorized` (non-authoritative probe)
//! - Creator returns `true`.
//! - Authorized delegate returns `true`.
//! - Stranger returns `false`.
//! - Missing token returns `false` (never panics).
//! - Returns `false` after delegate is revoked.
//!
//! ## Cross-cutting
//! - Creator can also hold a delegate on a different token.
//! - Multiple tokens with different creators are independently guarded.

#![cfg(test)]

use clips_nft::creator_auth_storage;
use clips_nft::creator_guard::{is_creator_or_authorized, require_creator, require_creator_or_authorized};
use clips_nft::{AtomicMintContract, Error, TokenId};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── Test harness ─────────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

const TOKEN: TokenId = 1;
const TOKEN2: TokenId = 2;

/// Seed a creator record for `token_id`.
fn seed_creator(env: &Env, token_id: TokenId, creator: &Address) {
    clips_nft::creator_storage::set_creator(env, token_id, creator);
}

/// Seed a creator AND an authorized delegate for `token_id`.
fn seed_creator_with_delegate(env: &Env, token_id: TokenId, creator: &Address, delegate: &Address) {
    seed_creator(env, token_id, creator);
    creator_auth_storage::set_authorized_account(env, token_id, delegate);
}

// ════════════════════════════════════════════════════════════════════════════
// require_creator — creator-only guard
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn creator_passes_require_creator() {
    with_contract(|env| {
        let creator = Address::generate(env);
        seed_creator(env, TOKEN, &creator);

        assert!(
            require_creator(env, &creator, TOKEN).is_ok(),
            "registered creator must pass require_creator"
        );
    });
}

#[test]
fn stranger_is_rejected_by_require_creator() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let stranger = Address::generate(env);
        seed_creator(env, TOKEN, &creator);

        assert_eq!(
            require_creator(env, &stranger, TOKEN),
            Err(Error::Unauthorized),
            "unrelated caller must be rejected by require_creator"
        );
    });
}

#[test]
fn authorized_delegate_is_rejected_by_require_creator() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &delegate);

        assert_eq!(
            require_creator(env, &delegate, TOKEN),
            Err(Error::Unauthorized),
            "authorized delegate must not pass the creator-only guard"
        );
    });
}

#[test]
fn missing_token_returns_not_found_in_require_creator() {
    with_contract(|env| {
        let caller = Address::generate(env);

        assert_eq!(
            require_creator(env, &caller, 999),
            Err(Error::TokenNotFound),
            "missing token record must propagate TokenNotFound"
        );
    });
}

// ════════════════════════════════════════════════════════════════════════════
// require_creator_or_authorized — creator OR delegate
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn creator_passes_require_creator_or_authorized() {
    with_contract(|env| {
        let creator = Address::generate(env);
        seed_creator(env, TOKEN, &creator);

        assert!(
            require_creator_or_authorized(env, &creator, TOKEN).is_ok(),
            "registered creator must pass require_creator_or_authorized"
        );
    });
}

#[test]
fn authorized_delegate_passes_require_creator_or_authorized() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &delegate);

        assert!(
            require_creator_or_authorized(env, &delegate, TOKEN).is_ok(),
            "authorized delegate must pass require_creator_or_authorized"
        );
    });
}

#[test]
fn unauthorized_third_party_rejected_by_require_creator_or_authorized() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        let stranger = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &delegate);

        assert_eq!(
            require_creator_or_authorized(env, &stranger, TOKEN),
            Err(Error::Unauthorized),
            "unrelated caller must be rejected by require_creator_or_authorized"
        );
    });
}

#[test]
fn missing_token_returns_not_found_in_require_creator_or_authorized() {
    with_contract(|env| {
        let caller = Address::generate(env);

        assert_eq!(
            require_creator_or_authorized(env, &caller, 999),
            Err(Error::TokenNotFound),
            "missing token record must propagate TokenNotFound"
        );
    });
}

#[test]
fn revoked_delegate_is_rejected_by_require_creator_or_authorized() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &delegate);

        // Revoke the authorization.
        creator_auth_storage::remove_authorized_account(env, TOKEN);

        assert_eq!(
            require_creator_or_authorized(env, &delegate, TOKEN),
            Err(Error::Unauthorized),
            "revoked delegate must no longer pass the guard"
        );
    });
}

#[test]
fn replaced_delegate_old_rejected_new_accepted() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let old_delegate = Address::generate(env);
        let new_delegate = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &old_delegate);

        // Replace with a new delegate.
        creator_auth_storage::set_authorized_account(env, TOKEN, &new_delegate);

        assert_eq!(
            require_creator_or_authorized(env, &old_delegate, TOKEN),
            Err(Error::Unauthorized),
            "old delegate must be rejected after replacement"
        );
        assert!(
            require_creator_or_authorized(env, &new_delegate, TOKEN).is_ok(),
            "new delegate must be accepted"
        );
    });
}

#[test]
fn authorization_is_isolated_per_token() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        seed_creator(env, TOKEN, &creator);
        seed_creator(env, TOKEN2, &creator);
        // Authorize delegate for TOKEN only.
        creator_auth_storage::set_authorized_account(env, TOKEN, &delegate);

        assert!(
            require_creator_or_authorized(env, &delegate, TOKEN).is_ok(),
            "delegate authorized for token 1 must pass for token 1"
        );
        assert_eq!(
            require_creator_or_authorized(env, &delegate, TOKEN2),
            Err(Error::Unauthorized),
            "delegate authorized for token 1 must NOT be accepted for token 2"
        );
    });
}

// ════════════════════════════════════════════════════════════════════════════
// is_creator_or_authorized — non-authoritative probe
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn probe_returns_true_for_creator() {
    with_contract(|env| {
        let creator = Address::generate(env);
        seed_creator(env, TOKEN, &creator);

        assert!(
            is_creator_or_authorized(env, &creator, TOKEN),
            "probe must return true for the registered creator"
        );
    });
}

#[test]
fn probe_returns_true_for_authorized_delegate() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &delegate);

        assert!(
            is_creator_or_authorized(env, &delegate, TOKEN),
            "probe must return true for the authorized delegate"
        );
    });
}

#[test]
fn probe_returns_false_for_stranger() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let stranger = Address::generate(env);
        seed_creator(env, TOKEN, &creator);

        assert!(
            !is_creator_or_authorized(env, &stranger, TOKEN),
            "probe must return false for an unrelated caller"
        );
    });
}

#[test]
fn probe_returns_false_for_missing_token_without_panicking() {
    with_contract(|env| {
        let caller = Address::generate(env);

        // Must not panic — missing token returns false.
        assert!(
            !is_creator_or_authorized(env, &caller, 999),
            "probe must return false for a non-existent token"
        );
    });
}

#[test]
fn probe_returns_false_after_delegate_revoked() {
    with_contract(|env| {
        let creator = Address::generate(env);
        let delegate = Address::generate(env);
        seed_creator_with_delegate(env, TOKEN, &creator, &delegate);
        creator_auth_storage::remove_authorized_account(env, TOKEN);

        assert!(
            !is_creator_or_authorized(env, &delegate, TOKEN),
            "probe must return false after delegate is revoked"
        );
    });
}

// ════════════════════════════════════════════════════════════════════════════
// Cross-cutting scenarios
// ════════════════════════════════════════════════════════════════════════════

/// The creator of token 1 can also be the authorized delegate on token 2
/// (when token 2 has a different creator).
#[test]
fn creator_of_one_token_can_be_delegate_of_another() {
    with_contract(|env| {
        let alice = Address::generate(env);
        let bob = Address::generate(env);

        // Alice is the creator of token 1.
        seed_creator(env, TOKEN, &alice);
        // Bob is the creator of token 2, but authorizes Alice as delegate.
        seed_creator(env, TOKEN2, &bob);
        creator_auth_storage::set_authorized_account(env, TOKEN2, &alice);

        assert!(
            require_creator_or_authorized(env, &alice, TOKEN).is_ok(),
            "alice must pass as creator of token 1"
        );
        assert!(
            require_creator_or_authorized(env, &alice, TOKEN2).is_ok(),
            "alice must also pass as delegate for token 2"
        );
        assert!(
            require_creator_or_authorized(env, &bob, TOKEN2).is_ok(),
            "bob must pass as creator of token 2"
        );
        assert_eq!(
            require_creator_or_authorized(env, &bob, TOKEN),
            Err(Error::Unauthorized),
            "bob must not be accepted for token 1 (no creator or delegate relation)"
        );
    });
}

/// Multiple tokens with independent creator + delegate relationships are each
/// guarded correctly.
#[test]
fn multiple_tokens_with_independent_creators_and_delegates() {
    with_contract(|env| {
        let creator_a = Address::generate(env);
        let creator_b = Address::generate(env);
        let delegate_a = Address::generate(env);
        let delegate_b = Address::generate(env);
        let interloper = Address::generate(env);

        seed_creator_with_delegate(env, TOKEN, &creator_a, &delegate_a);
        seed_creator_with_delegate(env, TOKEN2, &creator_b, &delegate_b);

        // Token 1: creator_a and delegate_a authorized; creator_b is not.
        assert!(require_creator_or_authorized(env, &creator_a, TOKEN).is_ok());
        assert!(require_creator_or_authorized(env, &delegate_a, TOKEN).is_ok());
        assert_eq!(
            require_creator_or_authorized(env, &creator_b, TOKEN),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            require_creator_or_authorized(env, &delegate_b, TOKEN),
            Err(Error::Unauthorized)
        );

        // Token 2: creator_b and delegate_b authorized; creator_a is not.
        assert!(require_creator_or_authorized(env, &creator_b, TOKEN2).is_ok());
        assert!(require_creator_or_authorized(env, &delegate_b, TOKEN2).is_ok());
        assert_eq!(
            require_creator_or_authorized(env, &creator_a, TOKEN2),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            require_creator_or_authorized(env, &delegate_a, TOKEN2),
            Err(Error::Unauthorized)
        );

        // A total interloper is rejected for both tokens.
        assert_eq!(
            require_creator_or_authorized(env, &interloper, TOKEN),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            require_creator_or_authorized(env, &interloper, TOKEN2),
            Err(Error::Unauthorized)
        );
    });
}
