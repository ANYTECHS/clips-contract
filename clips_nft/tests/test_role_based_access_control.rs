//! Integration tests for Role-Based Access Control (issue #1254).
//!
//! Verifies that [`clips_nft::role_based_access_control`] correctly defines
//! roles, maps them to permitted actions, and validates callers before
//! protected operations execute.

#![cfg(test)]

use clips_nft::{
    administrator_storage,
    mint_authorization,
    role_based_access_control::{
        caller_may_perform, check_caller_action, require_role, resolve_role,
        role_permits_action, ContractAction, ContractRole, RoleContext,
    },
    AtomicMintContract, Error,
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

fn setup_token_roles(env: &Env, token_id: u32, creator: &Address, owner: &Address) {
    clips_nft::creator_storage::set_creator(env, token_id, creator);
    clips_nft::token_owner_storage::save_owner(env, token_id, owner);
}

// ─── AC1: Defined supported roles ─────────────────────────────────────────────

#[test]
fn all_five_roles_are_distinct() {
    // Each variant must be distinguishable — equality must not collapse across roles.
    assert_ne!(ContractRole::Admin, ContractRole::Minter);
    assert_ne!(ContractRole::Admin, ContractRole::Creator);
    assert_ne!(ContractRole::Admin, ContractRole::Owner);
    assert_ne!(ContractRole::Admin, ContractRole::User);
    assert_ne!(ContractRole::Minter, ContractRole::Creator);
    assert_ne!(ContractRole::Minter, ContractRole::Owner);
    assert_ne!(ContractRole::Minter, ContractRole::User);
    assert_ne!(ContractRole::Creator, ContractRole::Owner);
    assert_ne!(ContractRole::Creator, ContractRole::User);
    assert_ne!(ContractRole::Owner, ContractRole::User);
}

#[test]
fn roles_resolve_for_each_identity_type() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        let user = Address::generate(env);
        setup_admin(env, &admin);
        mint_authorization::set_approved_minter(env, &minter);
        setup_token_roles(env, 1, &creator, &owner);

        assert_eq!(resolve_role(env, &admin, None).unwrap(), ContractRole::Admin);
        assert_eq!(resolve_role(env, &minter, None).unwrap(), ContractRole::Minter);
        assert_eq!(
            resolve_role(env, &creator, Some(&RoleContext::Token(1))).unwrap(),
            ContractRole::Creator
        );
        assert_eq!(
            resolve_role(env, &owner, Some(&RoleContext::Token(1))).unwrap(),
            ContractRole::Owner
        );
        assert_eq!(resolve_role(env, &user, None).unwrap(), ContractRole::User);
    });
}

// ─── AC2: Roles associated with permitted actions ─────────────────────────────

#[test]
fn admin_is_permitted_for_every_action() {
    let actions = [
        ContractAction::ConfigureContract,
        ContractAction::PauseContract,
        ContractAction::MintToken,
        ContractAction::BurnToken,
        ContractAction::TransferToken,
        ContractAction::UpdateMetadata,
        ContractAction::SetRoyalty,
        ContractAction::FreezeToken,
        ContractAction::ManageBlacklist,
        ContractAction::ApproveTransfer,
        ContractAction::ViewData,
    ];
    for action in &actions {
        assert!(
            role_permits_action(&ContractRole::Admin, action),
            "Admin denied {:?} — should be permitted",
            action
        );
    }
}

#[test]
fn minter_is_permitted_only_for_mint_and_view() {
    assert!(role_permits_action(&ContractRole::Minter, &ContractAction::MintToken));
    assert!(role_permits_action(&ContractRole::Minter, &ContractAction::ViewData));

    let denied = [
        ContractAction::ConfigureContract,
        ContractAction::PauseContract,
        ContractAction::BurnToken,
        ContractAction::TransferToken,
        ContractAction::UpdateMetadata,
        ContractAction::SetRoyalty,
        ContractAction::FreezeToken,
        ContractAction::ManageBlacklist,
        ContractAction::ApproveTransfer,
    ];
    for action in &denied {
        assert!(
            !role_permits_action(&ContractRole::Minter, action),
            "Minter unexpectedly permitted {:?}",
            action
        );
    }
}

#[test]
fn creator_permissions_match_expected_set() {
    let allowed = [
        ContractAction::BurnToken,
        ContractAction::TransferToken,
        ContractAction::UpdateMetadata,
        ContractAction::SetRoyalty,
        ContractAction::ViewData,
    ];
    for action in &allowed {
        assert!(
            role_permits_action(&ContractRole::Creator, action),
            "Creator denied {:?}",
            action
        );
    }

    let denied = [
        ContractAction::ConfigureContract,
        ContractAction::PauseContract,
        ContractAction::MintToken,
        ContractAction::FreezeToken,
        ContractAction::ManageBlacklist,
        ContractAction::ApproveTransfer,
    ];
    for action in &denied {
        assert!(
            !role_permits_action(&ContractRole::Creator, action),
            "Creator unexpectedly permitted {:?}",
            action
        );
    }
}

#[test]
fn owner_permissions_match_expected_set() {
    let allowed = [
        ContractAction::BurnToken,
        ContractAction::TransferToken,
        ContractAction::SetRoyalty,
        ContractAction::ApproveTransfer,
        ContractAction::ViewData,
    ];
    for action in &allowed {
        assert!(
            role_permits_action(&ContractRole::Owner, action),
            "Owner denied {:?}",
            action
        );
    }

    let denied = [
        ContractAction::ConfigureContract,
        ContractAction::PauseContract,
        ContractAction::MintToken,
        ContractAction::UpdateMetadata,
        ContractAction::FreezeToken,
        ContractAction::ManageBlacklist,
    ];
    for action in &denied {
        assert!(
            !role_permits_action(&ContractRole::Owner, action),
            "Owner unexpectedly permitted {:?}",
            action
        );
    }
}

#[test]
fn user_is_permitted_only_to_view() {
    assert!(role_permits_action(&ContractRole::User, &ContractAction::ViewData));

    let denied = [
        ContractAction::ConfigureContract,
        ContractAction::PauseContract,
        ContractAction::MintToken,
        ContractAction::BurnToken,
        ContractAction::TransferToken,
        ContractAction::UpdateMetadata,
        ContractAction::SetRoyalty,
        ContractAction::FreezeToken,
        ContractAction::ManageBlacklist,
        ContractAction::ApproveTransfer,
    ];
    for action in &denied {
        assert!(
            !role_permits_action(&ContractRole::User, action),
            "User unexpectedly permitted {:?}",
            action
        );
    }
}

// ─── AC3: Validate roles before protected operations ─────────────────────────

#[test]
fn admin_can_pause_the_contract() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let role = check_caller_action(env, &admin, ContractAction::PauseContract, None)
            .expect("admin should be permitted to pause");
        assert_eq!(role, ContractRole::Admin);
    });
}

#[test]
fn minter_can_mint_but_not_configure() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        setup_admin(env, &admin);
        mint_authorization::set_approved_minter(env, &minter);

        assert!(check_caller_action(env, &minter, ContractAction::MintToken, None).is_ok());
        assert_eq!(
            check_caller_action(env, &minter, ContractAction::ConfigureContract, None),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn creator_can_update_metadata_but_not_freeze() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        setup_admin(env, &admin);
        setup_token_roles(env, 1, &creator, &owner);

        assert!(check_caller_action(
            env,
            &creator,
            ContractAction::UpdateMetadata,
            Some(&RoleContext::Token(1))
        )
        .is_ok());

        assert_eq!(
            check_caller_action(
                env,
                &creator,
                ContractAction::FreezeToken,
                Some(&RoleContext::Token(1))
            ),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn owner_can_approve_transfer_but_not_update_metadata() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        setup_admin(env, &admin);
        setup_token_roles(env, 1, &creator, &owner);

        assert!(check_caller_action(
            env,
            &owner,
            ContractAction::ApproveTransfer,
            Some(&RoleContext::Token(1))
        )
        .is_ok());

        assert_eq!(
            check_caller_action(
                env,
                &owner,
                ContractAction::UpdateMetadata,
                Some(&RoleContext::Token(1))
            ),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn user_can_view_but_not_transfer() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let stranger = Address::generate(env);
        setup_admin(env, &admin);

        assert!(check_caller_action(env, &stranger, ContractAction::ViewData, None).is_ok());
        assert_eq!(
            check_caller_action(env, &stranger, ContractAction::TransferToken, None),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn uninitialized_contract_returns_not_initialized() {
    with_contract(|env| {
        let caller = Address::generate(env);
        assert_eq!(
            check_caller_action(env, &caller, ContractAction::ViewData, None),
            Err(Error::NotInitialized)
        );
    });
}

// ─── AC4: Role authorization tests — edge cases ──────────────────────────────

#[test]
fn admin_priority_over_all_other_roles() {
    with_contract(|env| {
        // An address that qualifies as all roles simultaneously should resolve
        // to Admin (the highest priority).
        let primary = Address::generate(env);
        let all_roles = Address::generate(env);
        setup_admin(env, &primary);

        // Register as multi-admin (Admin role)
        administrator_storage::add_admin(env, &all_roles);
        // Also register as minter
        mint_authorization::set_approved_minter(env, &all_roles);
        // Also set as creator and owner of token 1
        setup_token_roles(env, 1, &all_roles, &all_roles);

        let role = resolve_role(env, &all_roles, Some(&RoleContext::Token(1))).unwrap();
        assert_eq!(role, ContractRole::Admin, "Admin should win priority race");
    });
}

#[test]
fn non_authoritative_probe_does_not_consume_auth_envelope() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        setup_admin(env, &admin);
        mint_authorization::set_approved_minter(env, &minter);

        // caller_may_perform must work without mock_all_auths consuming envelopes
        assert!(caller_may_perform(env, &minter, &ContractAction::MintToken, None).unwrap());
        assert!(!caller_may_perform(env, &minter, &ContractAction::FreezeToken, None).unwrap());
    });
}

#[test]
fn role_revocation_immediately_denies_access() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        setup_admin(env, &admin);
        mint_authorization::set_approved_minter(env, &minter);

        // Granted
        assert!(check_caller_action(env, &minter, ContractAction::MintToken, None).is_ok());

        // Revoke
        mint_authorization::remove_approved_minter(env, &minter);

        // Now falls through to User — denied
        assert_eq!(
            check_caller_action(env, &minter, ContractAction::MintToken, None),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn multi_admin_revocation_immediately_denies_admin_actions() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi);

        assert!(check_caller_action(env, &multi, ContractAction::ManageBlacklist, None).is_ok());

        administrator_storage::remove_admin(env, &multi);

        assert_eq!(
            check_caller_action(env, &multi, ContractAction::ManageBlacklist, None),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn require_role_returns_correct_role_for_every_identity() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        let stranger = Address::generate(env);
        setup_admin(env, &admin);
        mint_authorization::set_approved_minter(env, &minter);

        assert_eq!(require_role(env, &admin, None).unwrap(), ContractRole::Admin);
        assert_eq!(require_role(env, &minter, None).unwrap(), ContractRole::Minter);
        assert_eq!(require_role(env, &stranger, None).unwrap(), ContractRole::User);
    });
}

#[test]
fn check_caller_action_composes_with_guard_builder() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::GuardBuilder::new()
            .add(
                check_caller_action(env, &admin, ContractAction::ConfigureContract, None)
                    .map(|_| ()),
            )
            .add(Ok(()))
            .execute();
        assert!(result.is_ok());
    });
}

#[test]
fn unauthorized_check_halts_guard_builder() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        setup_admin(env, &admin);
        mint_authorization::set_approved_minter(env, &minter);

        let result = clips_nft::guard_composition::GuardBuilder::new()
            .add(
                check_caller_action(env, &minter, ContractAction::PauseContract, None)
                    .map(|_| ()),
            )
            .add(Ok(()))
            .execute();
        assert_eq!(result, Err(Error::Unauthorized));
    });
}
