//! Integration tests for role assignment (issue #1256).
//!
//! Verifies that [`clips_nft::role_assignment`] correctly restricts role
//! grants and revocations to authorized managers, validates target addresses
//! and role identifiers, and prevents unauthorized role assignments.

#![cfg(test)]

use clips_nft::{
    administrator_storage,
    minter_role_storage,
    role_assignment::{
        assign_role, has_role, require_role_manager, revoke_role,
        validate_assignment_target, AssignableRole,
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

// ─── AC1: Only authorized role managers may assign roles ──────────────────────

#[test]
fn contract_admin_is_authorized_to_assign_roles() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        assert!(assign_role(env, &admin, &target, AssignableRole::Minter).is_ok());
    });
}

#[test]
fn multi_admin_is_authorized_to_assign_roles() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let multi = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &primary);
        administrator_storage::add_admin(env, &multi);

        assert!(assign_role(env, &multi, &target, AssignableRole::Minter).is_ok());
    });
}

#[test]
fn non_admin_cannot_assign_any_role() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        for role in AssignableRole::all() {
            assert_eq!(
                assign_role(env, &intruder, &target, role.clone()),
                Err(Error::Unauthorized),
                "non-admin must be denied assigning {:?}",
                role
            );
        }
    });
}

#[test]
fn minter_is_not_a_role_manager() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);
        assign_role(env, &admin, &minter, AssignableRole::Minter).unwrap();

        // Minters cannot use their role to grant further roles
        assert_eq!(
            assign_role(env, &minter, &target, AssignableRole::Minter),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn require_role_manager_returns_not_initialized_on_uninitialized_contract() {
    with_contract(|env| {
        let caller = Address::generate(env);
        assert_eq!(
            require_role_manager(env, &caller),
            Err(Error::NotInitialized)
        );
    });
}

#[test]
fn not_initialized_takes_priority_over_unauthorized() {
    with_contract(|env| {
        // No DataKey::Admin set — must return NotInitialized, not Unauthorized
        let caller = Address::generate(env);
        administrator_storage::add_admin(env, &caller);
        assert_eq!(
            require_role_manager(env, &caller),
            Err(Error::NotInitialized)
        );
    });
}

// ─── AC2: Validate target addresses and role identifiers ──────────────────────

#[test]
fn valid_external_address_passes_validation() {
    with_contract(|env| {
        let manager = Address::generate(env);
        let target = Address::generate(env);
        assert!(validate_assignment_target(env, &manager, &target).is_ok());
    });
}

#[test]
fn contract_address_as_target_is_rejected() {
    with_contract(|env| {
        let manager = Address::generate(env);
        let contract = env.current_contract_address();
        assert_eq!(
            validate_assignment_target(env, &manager, &contract),
            Err(Error::InvalidAddress)
        );
    });
}

#[test]
fn blacklisted_target_is_rejected() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        use clips_nft::blacklist;
        clips_nft::blacklist::add_wallet(env, &target);

        assert_eq!(
            assign_role(env, &admin, &target, AssignableRole::Minter),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn self_assignment_is_rejected() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        for role in AssignableRole::all() {
            assert_eq!(
                assign_role(env, &admin, &admin, role.clone()),
                Err(Error::InvalidAddress),
                "self-assignment must be rejected for {:?}",
                role
            );
        }
    });
}

#[test]
fn both_assignable_roles_are_valid_identifiers() {
    // Assigning and querying both roles must succeed without panicking
    with_contract(|env| {
        let admin = Address::generate(env);
        let target_a = Address::generate(env);
        let target_b = Address::generate(env);
        setup_admin(env, &admin);

        assign_role(env, &admin, &target_a, AssignableRole::Minter).unwrap();
        assign_role(env, &admin, &target_b, AssignableRole::Admin).unwrap();

        assert!(has_role(env, &target_a, AssignableRole::Minter));
        assert!(has_role(env, &target_b, AssignableRole::Admin));
    });
}

// ─── AC3: Prevent unauthorized role assignments ───────────────────────────────

#[test]
fn unauthorized_assignment_does_not_write_to_storage() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        let _ = assign_role(env, &intruder, &target, AssignableRole::Minter);
        // Storage must be clean — no role granted
        assert!(!minter_role_storage::is_approved_minter(env, &target));
    });
}

#[test]
fn unauthorized_admin_assignment_does_not_write_to_storage() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        let _ = assign_role(env, &intruder, &target, AssignableRole::Admin);
        assert!(!administrator_storage::is_admin(env, &target));
    });
}

#[test]
fn blacklisted_target_assignment_does_not_write_to_storage() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        clips_nft::blacklist::add_wallet(env, &target);
        let _ = assign_role(env, &admin, &target, AssignableRole::Minter);

        assert!(!minter_role_storage::is_approved_minter(env, &target));
    });
}

#[test]
fn non_admin_cannot_revoke_existing_role() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);
        assign_role(env, &admin, &minter, AssignableRole::Minter).unwrap();

        assert_eq!(
            revoke_role(env, &intruder, &minter, AssignableRole::Minter),
            Err(Error::Unauthorized)
        );
        // Role must still be active after the failed revocation
        assert!(minter_role_storage::is_approved_minter(env, &minter));
    });
}

#[test]
fn revoked_manager_cannot_assign_roles() {
    with_contract(|env| {
        let primary = Address::generate(env);
        let delegate = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &primary);

        // Grant delegate admin, then revoke
        assign_role(env, &primary, &delegate, AssignableRole::Admin).unwrap();
        revoke_role(env, &primary, &delegate, AssignableRole::Admin).unwrap();

        // Delegate's manager privileges are gone
        assert_eq!(
            assign_role(env, &delegate, &target, AssignableRole::Minter),
            Err(Error::Unauthorized)
        );
    });
}

// ─── AC4: Unit tests — full lifecycle ─────────────────────────────────────────

#[test]
fn full_assign_and_revoke_lifecycle_for_minter() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        // Initially no role
        assert!(!has_role(env, &target, AssignableRole::Minter));

        // Assign
        assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();
        assert!(has_role(env, &target, AssignableRole::Minter));

        // Revoke
        revoke_role(env, &admin, &target, AssignableRole::Minter).unwrap();
        assert!(!has_role(env, &target, AssignableRole::Minter));

        // Re-assign
        assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();
        assert!(has_role(env, &target, AssignableRole::Minter));
    });
}

#[test]
fn full_assign_and_revoke_lifecycle_for_admin_role() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let delegate = Address::generate(env);
        setup_admin(env, &admin);

        assert!(!has_role(env, &delegate, AssignableRole::Admin));

        assign_role(env, &admin, &delegate, AssignableRole::Admin).unwrap();
        assert!(has_role(env, &delegate, AssignableRole::Admin));

        // Delegate can now assign roles as a manager
        let target = Address::generate(env);
        assign_role(env, &delegate, &target, AssignableRole::Minter).unwrap();
        assert!(has_role(env, &target, AssignableRole::Minter));

        // Revoke delegate's admin role
        revoke_role(env, &admin, &delegate, AssignableRole::Admin).unwrap();
        assert!(!has_role(env, &delegate, AssignableRole::Admin));

        // Delegate can no longer assign roles
        let another = Address::generate(env);
        assert_eq!(
            assign_role(env, &delegate, &another, AssignableRole::Minter),
            Err(Error::Unauthorized)
        );
    });
}

#[test]
fn multiple_minters_can_be_assigned_independently() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let minter_a = Address::generate(env);
        let minter_b = Address::generate(env);
        let minter_c = Address::generate(env);
        setup_admin(env, &admin);

        assign_role(env, &admin, &minter_a, AssignableRole::Minter).unwrap();
        assign_role(env, &admin, &minter_b, AssignableRole::Minter).unwrap();

        assert!(has_role(env, &minter_a, AssignableRole::Minter));
        assert!(has_role(env, &minter_b, AssignableRole::Minter));
        assert!(!has_role(env, &minter_c, AssignableRole::Minter));

        revoke_role(env, &admin, &minter_b, AssignableRole::Minter).unwrap();

        assert!(has_role(env, &minter_a, AssignableRole::Minter));
        assert!(!has_role(env, &minter_b, AssignableRole::Minter));
        assert!(!has_role(env, &minter_c, AssignableRole::Minter));
    });
}

#[test]
fn revoking_a_role_never_assigned_is_a_noop() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        // Should not error
        assert!(revoke_role(env, &admin, &target, AssignableRole::Minter).is_ok());
        assert!(revoke_role(env, &admin, &target, AssignableRole::Admin).is_ok());
    });
}

#[test]
fn roles_are_independent_per_address() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let addr_a = Address::generate(env);
        let addr_b = Address::generate(env);
        setup_admin(env, &admin);

        assign_role(env, &admin, &addr_a, AssignableRole::Minter).unwrap();

        assert!(has_role(env, &addr_a, AssignableRole::Minter));
        assert!(!has_role(env, &addr_b, AssignableRole::Minter));
    });
}

#[test]
fn roles_are_independent_per_type_on_same_address() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let target = Address::generate(env);
        setup_admin(env, &admin);

        assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();

        assert!(has_role(env, &target, AssignableRole::Minter));
        assert!(!has_role(env, &target, AssignableRole::Admin));
    });
}

#[test]
fn guard_composes_with_sequence() {
    with_contract(|env| {
        let admin = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::sequence(
            require_role_manager(env, &admin),
            Ok(()),
        );
        assert!(result.is_ok());
    });
}

#[test]
fn unauthorized_manager_stops_guard_composition() {
    with_contract(|env| {
        let admin = Address::generate(env);
        let intruder = Address::generate(env);
        setup_admin(env, &admin);

        let result = clips_nft::guard_composition::sequence(
            require_role_manager(env, &intruder),
            Ok(()),
        );
        assert_eq!(result, Err(Error::Unauthorized));
    });
}
