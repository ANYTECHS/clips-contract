//! Integration tests for the administrator role (issue #1255).
//!
//! Verifies that [`clips_nft::administrator_role`] correctly defines
//! administrator permissions, protects configuration and administrative
//! operations, and prevents unauthorized access.

#![cfg(test)]

use clips_nft::{
    administrator_role::{
        is_admin_operation_permitted, require_admin_for, require_administrator,
        AdminOperation, AdminPermission,
    },
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

// ─── AC1: Define administrator permissions ────────────────────────────────────

#[test]
fn admin_permission_role_name_is_administrator() {
    let perm = AdminPermission::describe();
    assert_eq!(perm.role_name, "Administrator");
}

#[test]
fn admin_permission_lists_all_ten_protected_operations() {
    let perm = AdminPermission::describe();
    assert_eq!(
        perm.permitted_operations.len(),
        10,
        "all 10 admin-only operations must appear in permitted_operations"
    );
}

#[test]
fn admin_permission_explicitly_documents_non_permissions() {
    let perm = AdminPermission::describe();
    assert!(
        !perm.denied_operations.is_empty(),
        "denied_operations must explicitly document what admins cannot do"
    );
    assert!(
        perm.denied_operations.iter().any(|d| d.contains("mint")),
        "minting must be listed as a non-admin operation"
    );
}

#[test]
fn all_admin_operations_have_distinct_names() {
    let ops = AdminOperation::all();
    let mut seen: alloc::vec::Vec<&str> = ops.iter().map(|o| o.name()).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen.len(),
        ops.len(),
        "every AdminOperation must have a unique name"
    );
}

#[test]
fn admin_operation_names_match_contract_entry_points() {
    // These names are the ground truth — they must match the actual
    // ClipsNftContract function names that call require_config_admin.
    assert_eq!(AdminOperation::SetConfig.name(), "set_config");
    assert_eq!(AdminOperation::SetDefaultRoyalty.name(), "set_default_royalty_bps");
    assert_eq!(AdminOperation::PauseContract.name(), "pause");
    assert_eq!(AdminOperation::UnpauseContract.name(), "unpause");
    assert_eq!(AdminOperation::FreezeToken.name(), "freeze_token");
    assert_eq!(AdminOperation::UnfreezeToken.name(), "unfreeze_token");
    assert_eq!(AdminOperation::ReassignCreator.name(), "reassign_creator");
    assert_eq!(AdminOperation::SetTokenRoyalty.name(), "set_royalty");
    assert_eq!(AdminOperation::AddCurrency.name(), "add_currency");
    assert_eq!(AdminOperation::RemoveCurrency.name(), "remove_currency");
}

// ─── AC2: Protect configuration and administrative operations ─────────────────

#[test]
fn contract_level_admin_is_permitted_for_every_operation() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        for op in AdminOperation::all() {
            assert!(
                require_admin_for(env, &admin, op.clone()).is_ok(),
                "admin must be permitted for: {}",
                op.name()
            );
        }
    });
}

#[test]
fn multi_admin_is_permitted_for_every_operation() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi);

        for op in AdminOperation::all() {
            assert!(
                require_admin_for(env, &multi, op.clone()).is_ok(),
                "multi-admin must be permitted for: {}",
                op.name()
            );
        }
    });
}

#[test]
fn admin_can_set_config() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::SetConfig).is_ok());
    });
}

#[test]
fn admin_can_set_default_royalty() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::SetDefaultRoyalty).is_ok());
    });
}

#[test]
fn admin_can_pause_and_unpause_contract() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::PauseContract).is_ok());
        assert!(require_admin_for(env, &admin, AdminOperation::UnpauseContract).is_ok());
    });
}

#[test]
fn admin_can_freeze_and_unfreeze_token() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::FreezeToken).is_ok());
        assert!(require_admin_for(env, &admin, AdminOperation::UnfreezeToken).is_ok());
    });
}

#[test]
fn admin_can_reassign_creator() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::ReassignCreator).is_ok());
    });
}

#[test]
fn admin_can_set_token_royalty() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::SetTokenRoyalty).is_ok());
    });
}

#[test]
fn admin_can_add_and_remove_currency() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_admin_for(env, &admin, AdminOperation::AddCurrency).is_ok());
        assert!(require_admin_for(env, &admin, AdminOperation::RemoveCurrency).is_ok());
    });
}

// ─── AC3: Prevent unauthorized administrator-only calls ───────────────────────

#[test]
fn non_admin_is_rejected_for_every_operation() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);

        for op in AdminOperation::all() {
            assert_eq!(
                require_admin_for(env, &intruder, op.clone()),
                Err(Error::Unauthorized),
                "non-admin must be rejected for: {}",
                op.name()
            );
        }
    });
}

#[test]
fn uninitialized_contract_returns_not_initialized_for_every_operation() {
    with_contract(|env| {
        let caller = Address::generate(env);

        for op in AdminOperation::all() {
            assert_eq!(
                require_admin_for(env, &caller, op.clone()),
                Err(Error::NotInitialized),
                "should return NotInitialized (not Unauthorized) for: {}",
                op.name()
            );
        }
    });
}

#[test]
fn not_initialized_takes_priority_over_unauthorized() {
    with_contract(|env| {
        // Multi-admin entry exists, but DataKey::Admin is absent.
        let caller = Address::generate(env);
        administrator_storage::add_admin(env, &caller);

        assert_eq!(
            require_administrator(env, &caller),
            Err(Error::NotInitialized),
            "NotInitialized must fire before Unauthorized"
        );
    });
}

#[test]
fn revoked_multi_admin_is_rejected_for_every_operation() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi);

        // Confirm access was granted
        assert!(require_administrator(env, &multi).is_ok());

        // Revoke
        administrator_storage::remove_admin(env, &multi);

        for op in AdminOperation::all() {
            assert_eq!(
                require_admin_for(env, &multi, op.clone()),
                Err(Error::Unauthorized),
                "revoked admin must be denied for: {}",
                op.name()
            );
        }
    });
}

#[test]
fn probe_correctly_reflects_revocation() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi);

        assert!(is_admin_operation_permitted(env, &multi));

        administrator_storage::remove_admin(env, &multi);

        assert!(!is_admin_operation_permitted(env, &multi));
    });
}

// ─── AC4: Role tests ──────────────────────────────────────────────────────────

#[test]
fn require_administrator_accepts_primary_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        assert!(require_administrator(env, &admin).is_ok());
    });
}

#[test]
fn require_administrator_accepts_multi_admin() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi);
        assert!(require_administrator(env, &multi).is_ok());
    });
}

#[test]
fn require_administrator_rejects_non_admin() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let stranger = Address::generate(env);
        setup_admin(env, &admin);
        assert_eq!(
            require_administrator(env, &stranger),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn require_administrator_rejects_uninitialized_contract() {
    with_contract(|env| {
        let caller = Address::generate(env);
        assert_eq!(
            require_administrator(env, &caller),
            Err(Error::NotInitialized)
        );
    });
}

#[test]
fn require_administrator_is_deterministic() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);
        for _ in 0..5 {
            assert!(require_administrator(env, &admin).is_ok());
        }
    });
}

#[test]
fn multiple_multi_admins_are_all_accepted() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi_1 = Address::generate(env);
        let multi_2 = Address::generate(env);
        let multi_3 = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi_1);
        administrator_storage::add_admin(env, &multi_2);
        administrator_storage::add_admin(env, &multi_3);

        for addr in [&primary, &multi_1, &multi_2, &multi_3] {
            assert!(
                require_administrator(env, addr).is_ok(),
                "every registered admin must be accepted"
            );
        }
    });
}

#[test]
fn guard_composes_with_sequence() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::sequence(
            require_admin_for(env, &admin, AdminOperation::SetConfig),
            Ok(()),
        );
        assert!(result.is_ok());
    });
}

#[test]
fn guard_composes_with_guard_builder() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::GuardBuilder::new()
            .add(require_admin_for(env, &admin, AdminOperation::PauseContract))
            .add(require_admin_for(env, &admin, AdminOperation::SetConfig))
            .execute();
        assert!(result.is_ok());
    });
}

#[test]
fn unauthorized_failure_stops_guard_composition() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::sequence(
            require_admin_for(env, &intruder, AdminOperation::FreezeToken),
            Ok(()),
        );
        assert_eq!(result, Err(Error::Unauthorized));
    });
}
