//! Role assignment (issue #1256).
//!
//! Implements controlled assignment and revocation of roles to authorized
//! accounts. Only a recognized role manager (the contract administrator) may
//! grant or revoke roles. Every assignment is preceded by validation of both
//! the manager's authority and the target address.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Allow only authorized role managers to assign roles | [`require_role_manager`] + all `assign_*` / `revoke_*` functions |
//! | Validate target addresses and role identifiers | [`validate_assignment_target`] + [`AssignableRole`] |
//! | Prevent unauthorized role assignments | Guards reject non-admin callers before any storage write |
//! | Add unit tests | `tests` module with comprehensive coverage |
//!
//! # Assignable roles
//!
//! Not all roles can be directly assigned — `Creator` and `Owner` are
//! derived implicitly from NFT records, not from an explicit grant. The
//! roles that *can* be assigned are captured by [`AssignableRole`]:
//!
//! | Role | Storage key | Who may assign |
//! |------|-------------|----------------|
//! | [`AssignableRole::Minter`] | `DataKey::ApprovedMinter(target)` | Admin |
//! | [`AssignableRole::Admin`] | `DataKey::Administrator(target)` | Admin |
//!
//! # Assignment lifecycle
//!
//! ```text
//! assign_role(env, manager, target, role)
//!   │
//!   ├─ 1. require_role_manager → contract initialized + manager is admin + require_auth
//!   ├─ 2. validate_assignment_target → not contract address, not blacklisted, not manager
//!   ├─ 3. write role to storage (idempotent)
//!   └─ Ok(())
//!
//! revoke_role(env, manager, target, role)
//!   │
//!   ├─ 1. require_role_manager
//!   ├─ 2. validate_assignment_target
//!   ├─ 3. remove role from storage (no-op if not assigned)
//!   └─ Ok(())
//! ```
//!
//! # Errors (in evaluation order)
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::NotInitialized`] | Contract has no admin configured. |
//! | [`Error::Unauthorized`] | Manager is not an authorized administrator. |
//! | [`Error::InvalidAddress`] | Target is the contract's own address. |
//! | [`Error::Unauthorized`] | Target is on the blacklist. |
//! | [`Error::InvalidAddress`] | Manager and target are the same address (self-assignment). |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Grant the Minter role to `target_wallet`
//! role_assignment::assign_role(&env, &admin, &target_wallet, AssignableRole::Minter)?;
//!
//! // Grant an additional admin
//! role_assignment::assign_role(&env, &admin, &delegate, AssignableRole::Admin)?;
//!
//! // Revoke the Minter role
//! role_assignment::revoke_role(&env, &admin, &target_wallet, AssignableRole::Minter)?;
//!
//! // Check assignment without auth side-effects
//! let has_role = role_assignment::has_role(&env, &target_wallet, AssignableRole::Minter);
//! ```

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::blacklist;
use crate::minter_role_storage;
use crate::types::{DataKey, Error};

// ─── Assignable role identifier — AC2 ────────────────────────────────────────

/// The subset of contract roles that can be explicitly assigned to an address.
///
/// `Creator` and `Owner` are derived implicitly from NFT records and cannot
/// be granted through role assignment. Only the roles in this enum are
/// controlled by this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssignableRole {
    /// Allows the target address to mint new NFTs.
    ///
    /// Stored at `DataKey::ApprovedMinter(address)` in persistent storage.
    Minter,

    /// Grants the target address full administrator privileges (multi-admin
    /// registry). Stored at `DataKey::Administrator(address)` in persistent
    /// storage.
    ///
    /// Note: This adds to the multi-admin list. It does not replace the
    /// contract-level admin stored at `DataKey::Admin`.
    Admin,
}

impl AssignableRole {
    /// Human-readable name of the role (useful for logs and error messages).
    pub fn name(&self) -> &'static str {
        match self {
            AssignableRole::Minter => "Minter",
            AssignableRole::Admin => "Admin",
        }
    }

    /// Return a slice of all assignable role variants.
    pub fn all() -> &'static [AssignableRole] {
        &[AssignableRole::Minter, AssignableRole::Admin]
    }
}

// ─── Role manager guard — AC1 ─────────────────────────────────────────────────

/// Verify that `manager` is authorized to assign or revoke roles.
///
/// A caller is a valid role manager when **all** of the following hold:
///
/// 1. The contract has been initialized (`DataKey::Admin` exists).
/// 2. `manager` has signed the invocation (`require_auth`).
/// 3. `manager` is the contract-level admin or a registered multi-admin.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
/// - [`Error::Unauthorized`] — caller is not an authorized administrator.
pub fn require_role_manager(env: &Env, manager: &Address) -> Result<(), Error> {
    // AC1 — initialization check before anything else
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }

    // Demand a signed authorization envelope
    manager.require_auth();

    // Membership check: multi-admin list first, then contract-level admin
    if is_authorized_admin(env, manager) {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

// ─── Target address validation — AC2 ─────────────────────────────────────────

/// Validate that `target` is an acceptable role assignment destination.
///
/// Rejects:
/// 1. The contract's own address — contracts cannot hold user-level roles.
/// 2. Blacklisted addresses — they are blocked from all contract interactions.
/// 3. Self-assignment (`manager == target`) — role managers cannot assign
///    roles to themselves; use `administrator_storage::add_admin` directly
///    for bootstrapping purposes.
///
/// # Errors
/// - [`Error::InvalidAddress`] — target is the contract itself.
/// - [`Error::Unauthorized`] — target is blacklisted.
/// - [`Error::InvalidAddress`] — target and manager are the same address.
pub fn validate_assignment_target(
    env: &Env,
    manager: &Address,
    target: &Address,
) -> Result<(), Error> {
    // Reject self-calls (contract address as target)
    if *target == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }

    // Reject blacklisted targets
    if blacklist::is_blacklisted(env, target) {
        return Err(Error::Unauthorized);
    }

    // Reject self-assignment
    if *target == *manager {
        return Err(Error::InvalidAddress);
    }

    Ok(())
}

// ─── Assignment — AC1 / AC2 ───────────────────────────────────────────────────

/// Assign `role` to `target`, authorized by `manager`.
///
/// Full lifecycle:
/// 1. Verify `manager` is an authorized role manager ([`require_role_manager`]).
/// 2. Validate `target` is acceptable ([`validate_assignment_target`]).
/// 3. Persist the role assignment (idempotent — reassigning an existing role
///    is a no-op at the storage level).
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
/// - [`Error::Unauthorized`] — manager is not an authorized administrator.
/// - [`Error::InvalidAddress`] — target is the contract address or equals manager.
/// - [`Error::Unauthorized`] — target is blacklisted.
///
/// # Example
///
/// ```rust,ignore
/// role_assignment::assign_role(&env, &admin, &wallet, AssignableRole::Minter)?;
/// ```
pub fn assign_role(
    env: &Env,
    manager: &Address,
    target: &Address,
    role: AssignableRole,
) -> Result<(), Error> {
    require_role_manager(env, manager)?;
    validate_assignment_target(env, manager, target)?;
    write_role(env, target, &role);
    Ok(())
}

/// Revoke `role` from `target`, authorized by `manager`.
///
/// Full lifecycle:
/// 1. Verify `manager` is an authorized role manager ([`require_role_manager`]).
/// 2. Validate `target` is acceptable ([`validate_assignment_target`]).
/// 3. Remove the role from storage (no-op if the role was never assigned).
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
/// - [`Error::Unauthorized`] — manager is not an authorized administrator.
/// - [`Error::InvalidAddress`] — target is the contract address or equals manager.
/// - [`Error::Unauthorized`] — target is blacklisted.
///
/// # Example
///
/// ```rust,ignore
/// role_assignment::revoke_role(&env, &admin, &wallet, AssignableRole::Minter)?;
/// ```
pub fn revoke_role(
    env: &Env,
    manager: &Address,
    target: &Address,
    role: AssignableRole,
) -> Result<(), Error> {
    require_role_manager(env, manager)?;
    validate_assignment_target(env, manager, target)?;
    erase_role(env, target, &role);
    Ok(())
}

// ─── Read-only probe — AC2 ────────────────────────────────────────────────────

/// Return `true` when `target` currently holds `role`.
///
/// Pure read — does not call `require_auth` or modify state. Safe to use for
/// routing decisions and UI state building.
pub fn has_role(env: &Env, target: &Address, role: AssignableRole) -> bool {
    match role {
        AssignableRole::Minter => minter_role_storage::is_approved_minter(env, target),
        AssignableRole::Admin => administrator_storage::is_admin(env, target),
    }
}

// ─── Internal storage helpers ─────────────────────────────────────────────────

/// Persist `role` for `target` in the appropriate storage location.
fn write_role(env: &Env, target: &Address, role: &AssignableRole) {
    match role {
        AssignableRole::Minter => minter_role_storage::add_approved_minter(env, target),
        AssignableRole::Admin => administrator_storage::add_admin(env, target),
    }
}

/// Remove `role` for `target` from storage. No-op if not assigned.
fn erase_role(env: &Env, target: &Address, role: &AssignableRole) {
    match role {
        AssignableRole::Minter => minter_role_storage::remove_approved_minter(env, target),
        AssignableRole::Admin => administrator_storage::remove_admin(env, target),
    }
}

/// Return `true` when `caller` is the contract-level admin or a registered
/// multi-admin. Does not call `require_auth`.
fn is_authorized_admin(env: &Env, caller: &Address) -> bool {
    if administrator_storage::is_admin(env, caller) {
        return true;
    }
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .map(|admin| *caller == admin)
        .unwrap_or(false)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::blacklist;
    use crate::minter_role_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ──────────────────────────────────────────────────────────

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
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    // ── AssignableRole catalogue ──────────────────────────────────────────────

    #[test]
    fn assignable_role_names_are_unique() {
        let roles = AssignableRole::all();
        let mut names: alloc::vec::Vec<&str> = roles.iter().map(|r| r.name()).collect();
        names.dedup();
        assert_eq!(names.len(), roles.len());
    }

    #[test]
    fn assignable_role_variants_are_distinct() {
        assert_ne!(AssignableRole::Minter, AssignableRole::Admin);
    }

    #[test]
    fn minter_role_name() {
        assert_eq!(AssignableRole::Minter.name(), "Minter");
    }

    #[test]
    fn admin_role_name() {
        assert_eq!(AssignableRole::Admin.name(), "Admin");
    }

    // ── AC1: require_role_manager — authorized callers ────────────────────────

    #[test]
    fn contract_level_admin_is_a_valid_role_manager() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            assert!(require_role_manager(env, &admin).is_ok());
        });
    }

    #[test]
    fn registered_multi_admin_is_a_valid_role_manager() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);
            assert!(require_role_manager(env, &multi).is_ok());
        });
    }

    #[test]
    fn non_admin_is_not_a_valid_role_manager() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_role_manager(env, &stranger),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn role_manager_check_returns_not_initialized_when_no_admin() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                require_role_manager(env, &caller),
                Err(Error::NotInitialized)
            );
        });
    }

    #[test]
    fn revoked_multi_admin_is_no_longer_a_valid_role_manager() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);

            assert!(require_role_manager(env, &multi).is_ok());

            administrator_storage::remove_admin(env, &multi);
            assert_eq!(
                require_role_manager(env, &multi),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── AC2: validate_assignment_target ───────────────────────────────────────

    #[test]
    fn valid_external_address_passes_target_validation() {
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
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_assignment_target(env, &manager, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn blacklisted_target_is_rejected() {
        with_contract(|env| {
            let manager = Address::generate(env);
            let target = Address::generate(env);
            blacklist::add_wallet(env, &target);
            assert_eq!(
                validate_assignment_target(env, &manager, &target),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn self_assignment_is_rejected() {
        with_contract(|env| {
            let manager = Address::generate(env);
            // manager and target are the same address
            assert_eq!(
                validate_assignment_target(env, &manager, &manager),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn target_passes_validation_after_blacklist_removal() {
        with_contract(|env| {
            let manager = Address::generate(env);
            let target = Address::generate(env);
            blacklist::add_wallet(env, &target);

            assert!(validate_assignment_target(env, &manager, &target).is_err());

            blacklist::remove_wallet(env, &target);
            assert!(validate_assignment_target(env, &manager, &target).is_ok());
        });
    }

    // ── AC1 / AC2: assign_role ────────────────────────────────────────────────

    #[test]
    fn admin_can_assign_minter_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            assert!(assign_role(env, &admin, &target, AssignableRole::Minter).is_ok());
            assert!(minter_role_storage::is_approved_minter(env, &target));
        });
    }

    #[test]
    fn admin_can_assign_admin_role_to_delegate() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let delegate = Address::generate(env);
            setup_admin(env, &admin);

            assert!(assign_role(env, &admin, &delegate, AssignableRole::Admin).is_ok());
            assert!(administrator_storage::is_admin(env, &delegate));
        });
    }

    #[test]
    fn multi_admin_can_assign_minter_role() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);

            assert!(assign_role(env, &multi, &target, AssignableRole::Minter).is_ok());
            assert!(minter_role_storage::is_approved_minter(env, &target));
        });
    }

    #[test]
    fn assigning_role_twice_is_idempotent() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            assert!(assign_role(env, &admin, &target, AssignableRole::Minter).is_ok());
            assert!(assign_role(env, &admin, &target, AssignableRole::Minter).is_ok());
            assert!(minter_role_storage::is_approved_minter(env, &target));
        });
    }

    #[test]
    fn assign_role_reflects_in_has_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            assert!(!has_role(env, &target, AssignableRole::Minter));
            assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();
            assert!(has_role(env, &target, AssignableRole::Minter));
        });
    }

    #[test]
    fn assign_admin_role_reflects_in_has_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let delegate = Address::generate(env);
            setup_admin(env, &admin);

            assert!(!has_role(env, &delegate, AssignableRole::Admin));
            assign_role(env, &admin, &delegate, AssignableRole::Admin).unwrap();
            assert!(has_role(env, &delegate, AssignableRole::Admin));
        });
    }

    // ── AC3: prevent unauthorized assignments ─────────────────────────────────

    #[test]
    fn non_admin_cannot_assign_minter_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            assert_eq!(
                assign_role(env, &intruder, &target, AssignableRole::Minter),
                Err(Error::Unauthorized)
            );
            assert!(!minter_role_storage::is_approved_minter(env, &target));
        });
    }

    #[test]
    fn non_admin_cannot_assign_admin_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            assert_eq!(
                assign_role(env, &intruder, &target, AssignableRole::Admin),
                Err(Error::Unauthorized)
            );
            assert!(!administrator_storage::is_admin(env, &target));
        });
    }

    #[test]
    fn minter_cannot_assign_any_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);
            minter_role_storage::add_approved_minter(env, &minter);

            // Minters are not role managers
            assert_eq!(
                assign_role(env, &minter, &target, AssignableRole::Minter),
                Err(Error::Unauthorized)
            );
            assert_eq!(
                assign_role(env, &minter, &target, AssignableRole::Admin),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn uninitialized_contract_prevents_any_assignment() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let target = Address::generate(env);

            assert_eq!(
                assign_role(env, &caller, &target, AssignableRole::Minter),
                Err(Error::NotInitialized)
            );
            assert_eq!(
                assign_role(env, &caller, &target, AssignableRole::Admin),
                Err(Error::NotInitialized)
            );
        });
    }

    #[test]
    fn cannot_assign_role_to_contract_address() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            let contract_addr = env.current_contract_address();

            assert_eq!(
                assign_role(env, &admin, &contract_addr, AssignableRole::Minter),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn cannot_assign_role_to_blacklisted_address() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);
            blacklist::add_wallet(env, &target);

            assert_eq!(
                assign_role(env, &admin, &target, AssignableRole::Minter),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn cannot_self_assign_any_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            // Admin cannot assign a role to themselves via this API
            assert_eq!(
                assign_role(env, &admin, &admin, AssignableRole::Minter),
                Err(Error::InvalidAddress)
            );
            assert_eq!(
                assign_role(env, &admin, &admin, AssignableRole::Admin),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn storage_is_not_written_when_assignment_fails() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);
            blacklist::add_wallet(env, &target);

            // Assignment must fail — storage must remain clean
            let _ = assign_role(env, &admin, &target, AssignableRole::Minter);
            assert!(!minter_role_storage::is_approved_minter(env, &target));
        });
    }

    // ── revoke_role ───────────────────────────────────────────────────────────

    #[test]
    fn admin_can_revoke_minter_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);
            assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();

            assert!(revoke_role(env, &admin, &target, AssignableRole::Minter).is_ok());
            assert!(!minter_role_storage::is_approved_minter(env, &target));
        });
    }

    #[test]
    fn admin_can_revoke_admin_role_from_delegate() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let delegate = Address::generate(env);
            setup_admin(env, &admin);
            assign_role(env, &admin, &delegate, AssignableRole::Admin).unwrap();

            assert!(revoke_role(env, &admin, &delegate, AssignableRole::Admin).is_ok());
            assert!(!administrator_storage::is_admin(env, &delegate));
        });
    }

    #[test]
    fn revoking_non_assigned_role_is_a_noop() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            // No role assigned — should not error
            assert!(revoke_role(env, &admin, &target, AssignableRole::Minter).is_ok());
            assert!(!minter_role_storage::is_approved_minter(env, &target));
        });
    }

    #[test]
    fn non_admin_cannot_revoke_minter_role() {
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
            // Role must still be assigned after failed revocation
            assert!(minter_role_storage::is_approved_minter(env, &minter));
        });
    }

    #[test]
    fn revoke_role_reflects_in_has_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);
            assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();

            assert!(has_role(env, &target, AssignableRole::Minter));
            revoke_role(env, &admin, &target, AssignableRole::Minter).unwrap();
            assert!(!has_role(env, &target, AssignableRole::Minter));
        });
    }

    #[test]
    fn reassign_after_revoke_restores_role() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target = Address::generate(env);
            setup_admin(env, &admin);

            assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();
            revoke_role(env, &admin, &target, AssignableRole::Minter).unwrap();
            assign_role(env, &admin, &target, AssignableRole::Minter).unwrap();

            assert!(has_role(env, &target, AssignableRole::Minter));
        });
    }

    // ── has_role ──────────────────────────────────────────────────────────────

    #[test]
    fn has_role_returns_false_before_assignment() {
        with_contract(|env| {
            let target = Address::generate(env);
            assert!(!has_role(env, &target, AssignableRole::Minter));
            assert!(!has_role(env, &target, AssignableRole::Admin));
        });
    }

    #[test]
    fn has_role_is_independent_per_role() {
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
    fn has_role_is_independent_per_address() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let target_a = Address::generate(env);
            let target_b = Address::generate(env);
            setup_admin(env, &admin);

            assign_role(env, &admin, &target_a, AssignableRole::Minter).unwrap();

            assert!(has_role(env, &target_a, AssignableRole::Minter));
            assert!(!has_role(env, &target_b, AssignableRole::Minter));
        });
    }

    // ── Guard composition ─────────────────────────────────────────────────────

    #[test]
    fn role_manager_guard_composes_with_sequence() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                require_role_manager(env, &admin),
                Ok(()),
            );
            assert!(result.is_ok());
        });
    }

    #[test]
    fn unauthorized_manager_stops_composition() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                require_role_manager(env, &intruder),
                Ok(()),
            );
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }

    #[test]
    fn role_manager_composes_with_guard_builder() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::GuardBuilder::new()
                .add(require_role_manager(env, &admin))
                .add(Ok(()))
                .execute();
            assert!(result.is_ok());
        });
    }
}
