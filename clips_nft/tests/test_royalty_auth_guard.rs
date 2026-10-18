//! Integration test suite for the Royalty Authorization Guard (issue #1028).
//!
//! These tests exercise [`crate::royalty_auth_guard`] through a fully
//! registered Soroban contract environment to ensure the guard integrates
//! correctly with live storage, the pause state, and the royalty lifecycle.
//!
//! # Coverage
//! - All four authorized identity tiers (multi-admin, contract admin, creator,
//!   owner) are accepted by `require_royalty_auth`.
//! - Every unauthorized/revoked identity is rejected.
//! - Paused contract rejects all callers regardless of identity.
//! - Frozen royalty rejects all callers regardless of identity.
//! - Missing token returns `TokenNotFound` before identity checks.
//! - `require_royalty_admin_auth` rejects creator and owner identities.
//! - `require_royalty_auth_no_token` accepts admin/multi-admin and rejects
//!   unknown callers without requiring a token to exist.

#![cfg(test)]

use clips_nft::royalty_auth_guard::{
    require_royalty_admin_auth, require_royalty_auth, require_royalty_auth_no_token,
};
use clips_nft::{administrator_storage, token_storage};
use clips_nft::{AtomicMintContract, DataKey, Error, Royalty, RoyaltyRecipient, TokenId};
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── Test harness ────────────────────────────────────────────────────────────

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

fn make_royalty(env: &Env) -> Royalty {
    Royalty {
        recipients: soroban_sdk::vec![
            env,
            RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 500,
            }
        ],
        asset_address: None,
    }
}

/// Seed a token with admin / creator / owner identities and an active royalty.
fn seed_token(
    env: &Env,
    token_id: TokenId,
    admin: &Address,
    creator: &Address,
    owner: &Address,
) {
    env.storage().instance().set(&DataKey::Admin, admin);
    clips_nft::creator_storage::set_creator(env, token_id, creator);
    clips_nft::token_owner_storage::save_owner(env, token_id, owner);
    token_storage::set_royalty(env, token_id, &make_royalty(env));
}

// ── require_royalty_auth: authorized callers ────────────────────────────────

#[test]
fn contract_admin_passes_guard() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert!(require_royalty_auth(env, &admin, TOKEN).is_ok());
    });
}

#[test]
fn creator_passes_guard() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert!(require_royalty_auth(env, &creator, TOKEN).is_ok());
    });
}

#[test]
fn owner_passes_guard() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert!(require_royalty_auth(env, &owner, TOKEN).is_ok());
    });
}

#[test]
fn registered_multi_admin_passes_guard() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        let multi_admin = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        administrator_storage::add_admin(env, &multi_admin);

        assert!(require_royalty_auth(env, &multi_admin, TOKEN).is_ok());
    });
}

// ── require_royalty_auth: unauthorized callers ──────────────────────────────

#[test]
fn interloper_is_rejected() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        let interloper = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert_eq!(
            require_royalty_auth(env, &interloper, TOKEN),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

#[test]
fn revoked_multi_admin_is_rejected() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        let multi_admin = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        administrator_storage::add_admin(env, &multi_admin);
        administrator_storage::remove_admin(env, &multi_admin);

        assert_eq!(
            require_royalty_auth(env, &multi_admin, TOKEN),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

// ── require_royalty_auth: operational guards ────────────────────────────────

#[test]
fn paused_contract_blocks_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        clips_nft::pause_state::save_pause_state(env, true);

        assert_eq!(
            require_royalty_auth(env, &admin, TOKEN),
            Err(Error::ContractPaused)
        );
    });
}

#[test]
fn paused_contract_blocks_creator() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        clips_nft::pause_state::save_pause_state(env, true);

        assert_eq!(
            require_royalty_auth(env, &creator, TOKEN),
            Err(Error::ContractPaused)
        );
    });
}

#[test]
fn paused_contract_blocks_owner() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        clips_nft::pause_state::save_pause_state(env, true);

        assert_eq!(
            require_royalty_auth(env, &owner, TOKEN),
            Err(Error::ContractPaused)
        );
    });
}

#[test]
fn frozen_royalty_blocks_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyFrozen(TOKEN), &true);

        assert_eq!(
            require_royalty_auth(env, &admin, TOKEN),
            Err(Error::RoyaltyFrozen)
        );
    });
}

#[test]
fn frozen_royalty_blocks_creator() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyFrozen(TOKEN), &true);

        assert_eq!(
            require_royalty_auth(env, &creator, TOKEN),
            Err(Error::RoyaltyFrozen)
        );
    });
}

#[test]
fn missing_token_returns_not_found() {
    with_contract(|env| {
        let admin = Address::generate(env);
        env.storage().instance().set(&DataKey::Admin, &admin);

        assert_eq!(
            require_royalty_auth(env, &admin, 999),
            Err(Error::TokenNotFound)
        );
    });
}

// ── require_royalty_admin_auth ──────────────────────────────────────────────

#[test]
fn admin_auth_accepts_contract_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert!(require_royalty_admin_auth(env, &admin, TOKEN).is_ok());
    });
}

#[test]
fn admin_auth_accepts_multi_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        let multi_admin = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        administrator_storage::add_admin(env, &multi_admin);

        assert!(require_royalty_admin_auth(env, &multi_admin, TOKEN).is_ok());
    });
}

#[test]
fn admin_auth_rejects_creator() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert_eq!(
            require_royalty_admin_auth(env, &creator, TOKEN),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

#[test]
fn admin_auth_rejects_owner() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);

        assert_eq!(
            require_royalty_admin_auth(env, &owner, TOKEN),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

#[test]
fn admin_auth_paused_rejects_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        clips_nft::pause_state::save_pause_state(env, true);

        assert_eq!(
            require_royalty_admin_auth(env, &admin, TOKEN),
            Err(Error::ContractPaused)
        );
    });
}

#[test]
fn admin_auth_frozen_rejects_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        seed_token(env, TOKEN, &admin, &creator, &owner);
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyFrozen(TOKEN), &true);

        assert_eq!(
            require_royalty_admin_auth(env, &admin, TOKEN),
            Err(Error::RoyaltyFrozen)
        );
    });
}

// ── require_royalty_auth_no_token ────────────────────────────────────────────

#[test]
fn no_token_accepts_contract_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        env.storage().instance().set(&DataKey::Admin, &admin);

        assert!(require_royalty_auth_no_token(env, &admin).is_ok());
    });
}

#[test]
fn no_token_accepts_multi_admin() {
    with_contract(|env| {
        let multi_admin = Address::generate(env);
        administrator_storage::add_admin(env, &multi_admin);

        assert!(require_royalty_auth_no_token(env, &multi_admin).is_ok());
    });
}

#[test]
fn no_token_rejects_unknown_caller() {
    with_contract(|env| {
        let caller = Address::generate(env);

        assert_eq!(
            require_royalty_auth_no_token(env, &caller),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

#[test]
fn no_token_paused_rejects_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        clips_nft::pause_state::save_pause_state(env, true);

        assert_eq!(
            require_royalty_auth_no_token(env, &admin),
            Err(Error::ContractPaused)
        );
    });
}
