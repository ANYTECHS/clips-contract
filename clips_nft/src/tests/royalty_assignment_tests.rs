//! Royalty assignment test suite (issue #797).
//!
//! Covers initial royalty assignment and royalty updates end-to-end:
//! - valid assignment
//! - invalid recipient
//! - invalid royalty
//! - nonexistent NFT
//! - unauthorized update
//! - frozen royalty
//! - maximum royalty
//!
//! Exercises the assignment functions in [`crate::mint_royalty_init`] and the
//! update guards in [`crate::royalty_validation_pipeline`].

#![cfg(test)]

use crate::mint_royalty_init::{initialize_nft_royalty, RoyaltyInitParams};
use crate::royalty_validation_pipeline::{
    validate_royalty_configuration, validate_royalty_operation, validate_royalty_state,
};
use crate::token_storage;
use crate::types::{DataKey, Error, Royalty, RoyaltyRecipient, TokenId};
use crate::AtomicMintContract;
use soroban_sdk::{
    testutils::Address as _, Address, Env, Vec,
};

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn assignment_royalty(env: &Env, recipient: &Address, bps: u32) -> RoyaltyInitParams {
    RoyaltyInitParams {
        recipients: Some(soroban_sdk::vec![env, RoyaltyRecipient {
            recipient: recipient.clone(),
            basis_points: bps,
        }]),
        asset_address: None,
    }
}

fn register_token(env: &Env, token_id: TokenId, admin: &Address, owner: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
    crate::creator_storage::set_creator(env, token_id, owner);
    crate::token_owner_storage::save_owner(env, token_id, owner);
    token_storage::set_royalty(env, token_id, &assign_royalty_value(env, 500));
}

// ── Valid assignment ────────────────────────────────────────────────────────

#[test]
fn valid_assignment_persists_royalty() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let recipient = Address::generate(env);
        register_token(env, 1, &owner, &owner);

        let royalty = initialize_nft_royalty(env, 1, &assignment_royalty(env, &recipient, 750), &owner).unwrap();
        assert_eq!(royalty.recipients.get(0).unwrap().recipient, recipient);
        assert_eq!(royalty.recipients.get(0).unwrap().basis_points, 750);

        let stored = token_storage::get_royalty(env, 1).unwrap();
        assert_eq!(stored.recipients.get(0).unwrap().recipient, recipient);

        assert!(validate_royalty_operation(env, &owner, 1, &stored).is_ok());
    });
}

// ── Invalid recipient ───────────────────────────────────────────────────────

#[test]
fn invalid_recipient_rejected_on_assignment() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let contract = env.current_contract_address();
        register_token(env, 2, &owner, &owner);

        assert_eq!(
            initialize_nft_royalty(env, 2, &assignment_royalty(env, &contract, 500), &owner),
            Err(Error::InvalidRecipient)
        );
    });
}

// ── Invalid royalty ─────────────────────────────────────────────────────────

#[test]
fn invalid_royalty_rejected() {
    with_contract(|env| {
        let owner = Address::generate(env);
        register_token(env, 3, &owner, &owner);

        let empty = Royalty {
            recipients: Vec::new(env),
            asset_address: None,
        };
        // Empty recipient set exceeds the pipeline's non-empty requirement.
        assert_eq!(validate_royalty_configuration(env, &empty), Err(Error::InvalidBasisPoints));

        let over_max = Royalty {
            recipients: soroban_sdk::vec![env, RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 10_001,
            }],
            asset_address: None,
        };
        assert_eq!(validate_royalty_configuration(env, &over_max), Err(Error::InvalidBasisPoints));
    });
}

// ── Nonexistent NFT ─────────────────────────────────────────────────────────

#[test]
fn nonexistent_nft_returns_not_found() {
    with_contract(|env| {
        let owner = Address::generate(env);
        env.storage().instance().set(&DataKey::Admin, &owner);

        assert_eq!(token_storage::get_royalty(env, 999), Err(Error::TokenNotFound));

        let fantasy = assignment_royalty(env, &Address::generate(env), 500);
        let royalty = Royalty {
            recipients: fantasy.recipients.unwrap(),
            asset_address: None,
        };
        assert_eq!(
            validate_royalty_operation(env, &owner, 999, &royalty),
            Err(Error::TokenNotFound)
        );
    });
}

// ── Unauthorized update ─────────────────────────────────────────────────────

#[test]
fn unauthorized_update_rejected() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let owner = Address::generate(env);
        let interloper = Address::generate(env);
        register_token(env, 5, &admin, &owner);

        let update = assign_royalty_value(env, 700);
        assert_eq!(
            validate_royalty_operation(env, &interloper, 5, &update),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

// ── Frozen royalty ──────────────────────────────────────────────────────────

#[test]
fn frozen_royalty_rejects_updates() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let owner = Address::generate(env);
        register_token(env, 6, &admin, &owner);
        token_storage::set_royalty(env, 6, &assign_royalty_value(env, 500));

        // Simulate the permanent freeze marker.
        env.storage().persistent().set(&DataKey::RoyaltyFrozen(6), &true);

        assert_eq!(validate_royalty_state(env, 6), Err(Error::RoyaltyFrozen));
    });
}

// ── Maximum royalty ─────────────────────────────────────────────────────────

#[test]
fn maximum_royalty_is_valid_but_above_max_is_rejected() {
    with_contract(|env| {
        let owner = Address::generate(env);
        let recipient = Address::generate(env);
        register_token(env, 7, &owner, &owner);

        let max = assign_royalty_value(env, 10_000);
        assert!(validate_royalty_configuration(env, &max).is_ok());

        let above = assign_royalty_value(env, 10_001);
        assert_eq!(
            validate_royalty_configuration(env, &above),
            Err(Error::InvalidBasisPoints)
        );

        let assigned = initialize_nft_royalty(env, 7, &assignment_royalty(env, &recipient, 10_000), &owner).unwrap();
        assert_eq!(assigned.recipients.get(0).unwrap().basis_points, 10_000);
    });
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn assign_royalty_value(env: &Env, bps: u32) -> Royalty {
    Royalty {
        recipients: soroban_sdk::vec![env, RoyaltyRecipient {
            recipient: Address::generate(env),
            basis_points: bps,
        }],
        asset_address: None,
    }
}