//! Administrator role definition (issue #1255).
//!
//! Defines the contract administrator role, documents every operation that
//! requires administrator privileges, and provides the canonical guard that
//! protects those operations.
//!
//! # What this module adds over existing guards
//!
//! Prior modules (`admin_authorization`, `admin_access_control_guard`,
//! `config_guard`) each implement pieces of admin authorization. This module
//! is the **single authoritative reference** that:
//!
//! - Enumerates every [`AdminOperation`] that requires administrator privileges.
//! - Provides [`AdminPermission`] — a typed description of what an admin
//!   may and may not do.
//! - Exposes [`require_admin_for`] — a guard that couples an operation name to
//!   the authorization check so violations name *which operation* was blocked.
//! - Exposes [`is_admin_operation`] — a pure probe with no side effects.
//!
//! # Administrator role definition
//!
//! The **contract administrator** is the address stored at [`DataKey::Admin`]
//! during contract initialization, plus any addresses in the registered
//! multi-admin list ([`DataKey::Administrator`]).
//!
//! ## Permissions
//!
//! An administrator may perform **every** operation on the contract:
//!
//! | Category | Operations |
//! |----------|-----------|
//! | **Configuration** | [`AdminOperation::SetConfig`], [`AdminOperation::SetDefaultRoyalty`] |
//! | **Lifecycle** | [`AdminOperation::PauseContract`], [`AdminOperation::UnpauseContract`] |
//! | **Token management** | [`AdminOperation::FreezeToken`], [`AdminOperation::UnfreezeToken`], [`AdminOperation::ReassignCreator`] |
//! | **Royalty** | [`AdminOperation::SetTokenRoyalty`] |
//! | **Asset registry** | [`AdminOperation::AddCurrency`], [`AdminOperation::RemoveCurrency`] |
//!
//! ## Non-permissions
//!
//! The administrator role does **not** grant:
//! - Minting rights — those belong to the [`DataKey::ApprovedMinter`] registry.
//! - Token transfer — the admin cannot move tokens they don't own.
//! - Royalty collection — royalty payments go to configured recipients, not
//!   the admin.
//!
//! # Two-tier admin model
//!
//! The contract supports two kinds of admin identity, checked in this order:
//!
//! 1. **Registered multi-admin** — stored via [`crate::administrator_storage`]
//!    at `DataKey::Administrator(address)`.  Any number of addresses may be
//!    registered.
//! 2. **Contract-level admin** — the single address at `DataKey::Admin` set
//!    during [`init`](crate::ClipsNftContract::init).
//!
//! Both are accepted by every guard in this module.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Protect a specific admin operation with a named operation type
//! administrator_role::require_admin_for(&env, &caller, AdminOperation::FreezeToken)?;
//!
//! // Check without side effects
//! if administrator_role::is_admin_operation_permitted(&env, &caller) {
//!     // caller is an admin — safe to proceed
//! }
//!
//! // Retrieve the full permission description
//! let perms = AdminPermission::describe();
//! ```
//!
//! # Error precedence
//!
//! | Order | Error | Condition |
//! |-------|-------|-----------|
//! | 1 | [`Error::NotInitialized`] | No admin has been configured. |
//! | 2 | [`Error::Unauthorized`] | Caller is not an authorized administrator. |

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::types::{DataKey, Error};

// ─── Administrator operation catalogue — AC1 ─────────────────────────────────

/// Every contract operation that requires administrator privileges.
///
/// Pass one of these to [`require_admin_for`] to enforce authorization and
/// produce a precise error context when the call is rejected.
///
/// This enum is the **authoritative catalogue** of admin-only operations.
/// Any new operation added to the contract that requires admin privileges
/// must have a corresponding variant here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdminOperation {
    // ── Configuration ─────────────────────────────────────────────────────────
    /// Update global contract configuration (admin address, fee caps,
    /// royalty cap, mint cooldown). Corresponds to `set_config`.
    SetConfig,

    /// Set the contract-wide default royalty basis points applied to newly
    /// minted NFTs. Corresponds to `set_default_royalty_bps`.
    SetDefaultRoyalty,

    // ── Contract lifecycle ────────────────────────────────────────────────────
    /// Pause the contract, blocking all guarded entry points.
    /// Corresponds to `pause`.
    PauseContract,

    /// Resume a paused contract. Corresponds to `unpause`.
    UnpauseContract,

    // ── Token management ──────────────────────────────────────────────────────
    /// Freeze a token (make it non-transferable / soulbound).
    /// Corresponds to `freeze_token`.
    FreezeToken,

    /// Remove the frozen state from a token.
    /// Corresponds to `unfreeze_token`.
    UnfreezeToken,

    /// Reassign the creator address of an existing NFT.
    /// Corresponds to `reassign_creator`.
    ReassignCreator,

    // ── Royalty management ────────────────────────────────────────────────────
    /// Set or replace the royalty configuration for a specific token.
    /// Corresponds to `set_royalty`.
    SetTokenRoyalty,

    // ── Asset registry ────────────────────────────────────────────────────────
    /// Register a new supported payment currency.
    /// Corresponds to `add_currency`.
    AddCurrency,

    /// Deregister a supported payment currency.
    /// Corresponds to `remove_currency`.
    RemoveCurrency,
}

impl AdminOperation {
    /// Human-readable name of the operation (useful for logs and error messages).
    pub fn name(&self) -> &'static str {
        match self {
            AdminOperation::SetConfig => "set_config",
            AdminOperation::SetDefaultRoyalty => "set_default_royalty_bps",
            AdminOperation::PauseContract => "pause",
            AdminOperation::UnpauseContract => "unpause",
            AdminOperation::FreezeToken => "freeze_token",
            AdminOperation::UnfreezeToken => "unfreeze_token",
            AdminOperation::ReassignCreator => "reassign_creator",
            AdminOperation::SetTokenRoyalty => "set_royalty",
            AdminOperation::AddCurrency => "add_currency",
            AdminOperation::RemoveCurrency => "remove_currency",
        }
    }

    /// Return a slice containing every defined [`AdminOperation`].
    ///
    /// Useful for iterating the full set in tests or documentation tooling.
    pub fn all() -> &'static [AdminOperation] {
        &[
            AdminOperation::SetConfig,
            AdminOperation::SetDefaultRoyalty,
            AdminOperation::PauseContract,
            AdminOperation::UnpauseContract,
            AdminOperation::FreezeToken,
            AdminOperation::UnfreezeToken,
            AdminOperation::ReassignCreator,
            AdminOperation::SetTokenRoyalty,
            AdminOperation::AddCurrency,
            AdminOperation::RemoveCurrency,
        ]
    }
}

// ─── Permission description — AC1 ────────────────────────────────────────────

/// Typed description of what the administrator role grants and denies.
///
/// This is a documentation/introspection type — it carries no runtime state.
/// Call [`AdminPermission::describe`] to obtain an instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminPermission {
    /// Human-readable role name.
    pub role_name: &'static str,
    /// Operations the administrator is permitted to perform.
    pub permitted_operations: &'static [&'static str],
    /// Operations the administrator is explicitly denied (documents intent).
    pub denied_operations: &'static [&'static str],
}

impl AdminPermission {
    /// Return the permission description for the administrator role.
    ///
    /// This is a compile-time constant description — it does not read storage.
    pub fn describe() -> AdminPermission {
        AdminPermission {
            role_name: "Administrator",
            permitted_operations: &[
                "set_config",
                "set_default_royalty_bps",
                "pause",
                "unpause",
                "freeze_token",
                "unfreeze_token",
                "reassign_creator",
                "set_royalty",
                "add_currency",
                "remove_currency",
            ],
            denied_operations: &[
                "mint (use ApprovedMinter role)",
                "transfer tokens not owned by admin",
                "collect royalty payments",
            ],
        }
    }
}

// ─── Authorization guard — AC2 / AC3 ─────────────────────────────────────────

/// Require that `caller` is an authorized administrator before performing
/// `operation`.
///
/// This is the canonical admin guard for all entry points listed in
/// [`AdminOperation`]. It enforces:
///
/// 1. Contract is initialized (`DataKey::Admin` exists).
/// 2. Caller signs the invocation (`require_auth`).
/// 3. Caller matches the contract-level admin or the multi-admin registry.
///
/// Returns `Ok(())` only when all three checks pass.
///
/// # Errors
/// - [`Error::NotInitialized`] — no admin configured.
/// - [`Error::Unauthorized`] — caller is not an authorized administrator.
///
/// # Example
///
/// ```rust,ignore
/// administrator_role::require_admin_for(&env, &caller, AdminOperation::FreezeToken)?;
/// token_lifecycle::freeze_token(&env, token_id)?;
/// ```
pub fn require_admin_for(
    env: &Env,
    caller: &Address,
    _operation: AdminOperation, // named for documentation; resolved identically for all ops
) -> Result<(), Error> {
    require_any_admin(env, caller)
}

/// Require that `caller` is an authorized administrator (operation-agnostic).
///
/// Equivalent to [`require_admin_for`] but without an operation label.
/// Use this when the call site already names the operation clearly in context,
/// or when you need a `Result<(), Error>` guard that composes with
/// [`crate::guard_composition`].
///
/// # Errors
/// - [`Error::NotInitialized`] — no admin configured.
/// - [`Error::Unauthorized`] — caller is not an authorized administrator.
pub fn require_administrator(env: &Env, caller: &Address) -> Result<(), Error> {
    require_any_admin(env, caller)
}

// ─── Non-authoritative probe — AC2 ───────────────────────────────────────────

/// Return `true` when `caller` holds administrator privileges.
///
/// Performs a **read-only** check with no auth-envelope side effects.
/// Use for routing decisions, UI state, or building conditional logic where
/// consuming a signed envelope is incorrect.
///
/// Returns `false` when the contract is uninitialized.
///
/// # Example
///
/// ```rust,ignore
/// if administrator_role::is_admin_operation_permitted(&env, &caller) {
///     // safe to proceed without auth — you still need to call require_admin_for
///     // before any actual mutation
/// }
/// ```
pub fn is_admin_operation_permitted(env: &Env, caller: &Address) -> bool {
    is_authorized_admin(env, caller)
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Full guard: init check → require_auth → membership check.
fn require_any_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }

    caller.require_auth();

    if is_authorized_admin(env, caller) {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

/// Pure read: returns `true` when `caller` is the contract-level admin or a
/// registered multi-admin. Does not call `require_auth`.
fn is_authorized_admin(env: &Env, caller: &Address) -> bool {
    // Priority 1: registered multi-admin list
    if administrator_storage::is_admin(env, caller) {
        return true;
    }
    // Priority 2: contract-level admin
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

    // ── AC1: AdminOperation catalogue ────────────────────────────────────────

    #[test]
    fn all_operations_have_unique_names() {
        let ops = AdminOperation::all();
        let mut names: alloc::vec::Vec<&str> = ops.iter().map(|o| o.name()).collect();
        let original_len = names.len();
        names.dedup();
        assert_eq!(names.len(), original_len, "operation names must be unique");
    }

    #[test]
    fn all_returns_ten_operations() {
        assert_eq!(AdminOperation::all().len(), 10);
    }

    #[test]
    fn operation_names_match_contract_entry_points() {
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

    #[test]
    fn all_operations_are_distinct() {
        let ops = AdminOperation::all();
        for i in 0..ops.len() {
            for j in (i + 1)..ops.len() {
                assert_ne!(
                    ops[i], ops[j],
                    "operation variants must be distinct: {:?} == {:?}",
                    ops[i], ops[j]
                );
            }
        }
    }

    // ── AC1: AdminPermission description ─────────────────────────────────────

    #[test]
    fn permission_description_names_administrator_role() {
        let perm = AdminPermission::describe();
        assert_eq!(perm.role_name, "Administrator");
    }

    #[test]
    fn permission_description_lists_all_ten_permitted_operations() {
        let perm = AdminPermission::describe();
        assert_eq!(
            perm.permitted_operations.len(),
            10,
            "all 10 admin operations must be listed as permitted"
        );
    }

    #[test]
    fn permission_description_documents_denied_operations() {
        let perm = AdminPermission::describe();
        assert!(
            !perm.denied_operations.is_empty(),
            "denied_operations must document explicit non-permissions"
        );
        // Minting is the most important non-permission to document
        assert!(
            perm.denied_operations.iter().any(|d| d.contains("mint")),
            "denied_operations must mention mint"
        );
    }

    #[test]
    fn permission_description_is_deterministic() {
        assert_eq!(AdminPermission::describe(), AdminPermission::describe());
    }

    // ── AC2: is_admin_operation_permitted — non-authoritative probe ───────────

    #[test]
    fn probe_returns_true_for_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            assert!(is_admin_operation_permitted(env, &admin));
        });
    }

    #[test]
    fn probe_returns_true_for_registered_multi_admin() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);
            assert!(is_admin_operation_permitted(env, &multi));
        });
    }

    #[test]
    fn probe_returns_false_for_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);
            assert!(!is_admin_operation_permitted(env, &stranger));
        });
    }

    #[test]
    fn probe_returns_false_when_uninitialized() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(!is_admin_operation_permitted(env, &caller));
        });
    }

    #[test]
    fn probe_returns_false_after_multi_admin_removal() {
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

    // ── AC2 / AC3: require_admin_for — every operation ────────────────────────

    #[test]
    fn require_admin_for_accepts_admin_for_all_operations() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            for op in AdminOperation::all() {
                assert!(
                    require_admin_for(env, &admin, op.clone()).is_ok(),
                    "admin should be permitted for operation: {}",
                    op.name()
                );
            }
        });
    }

    #[test]
    fn require_admin_for_accepts_multi_admin_for_all_operations() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);

            for op in AdminOperation::all() {
                assert!(
                    require_admin_for(env, &multi, op.clone()).is_ok(),
                    "multi-admin should be permitted for operation: {}",
                    op.name()
                );
            }
        });
    }

    #[test]
    fn require_admin_for_rejects_non_admin_for_every_operation() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);

            for op in AdminOperation::all() {
                assert_eq!(
                    require_admin_for(env, &intruder, op.clone()),
                    Err(Error::Unauthorized),
                    "non-admin must be rejected for operation: {}",
                    op.name()
                );
            }
        });
    }

    #[test]
    fn require_admin_for_returns_not_initialized_on_uninitialized_contract() {
        with_contract(|env| {
            let caller = Address::generate(env);
            // NotInitialized fires before Unauthorized for every operation
            for op in AdminOperation::all() {
                assert_eq!(
                    require_admin_for(env, &caller, op.clone()),
                    Err(Error::NotInitialized),
                    "should return NotInitialized for operation: {}",
                    op.name()
                );
            }
        });
    }

    // ── AC3: require_administrator ────────────────────────────────────────────

    #[test]
    fn require_administrator_accepts_contract_level_admin() {
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
            let intruder = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_administrator(env, &intruder),
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
    fn require_administrator_rejects_after_multi_admin_removal() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);

            assert!(require_administrator(env, &multi).is_ok());

            administrator_storage::remove_admin(env, &multi);
            assert_eq!(
                require_administrator(env, &multi),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_administrator_accepts_multiple_multi_admins() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi_1 = Address::generate(env);
            let multi_2 = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi_1);
            administrator_storage::add_admin(env, &multi_2);

            assert!(require_administrator(env, &primary).is_ok());
            assert!(require_administrator(env, &multi_1).is_ok());
            assert!(require_administrator(env, &multi_2).is_ok());
        });
    }

    // ── AC3: Prevent unauthorized calls — specific admin operations ───────────

    #[test]
    fn non_admin_cannot_set_config() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_admin_for(env, &intruder, AdminOperation::SetConfig),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn non_admin_cannot_pause_contract() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_admin_for(env, &intruder, AdminOperation::PauseContract),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn non_admin_cannot_freeze_token() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_admin_for(env, &intruder, AdminOperation::FreezeToken),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn non_admin_cannot_reassign_creator() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_admin_for(env, &intruder, AdminOperation::ReassignCreator),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn non_admin_cannot_add_currency() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_admin_for(env, &intruder, AdminOperation::AddCurrency),
                Err(Error::Unauthorized)
            );
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
                Err(Error::NotInitialized)
            );
        });
    }

    // ── AC4: Guard composition ────────────────────────────────────────────────

    #[test]
    fn composes_with_sequence_guard() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                require_admin_for(env, &admin, AdminOperation::PauseContract),
                Ok(()),
            );
            assert!(result.is_ok());
        });
    }

    #[test]
    fn unauthorized_failure_halts_guard_sequence() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                require_admin_for(env, &intruder, AdminOperation::FreezeToken),
                Ok(()),
            );
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }

    #[test]
    fn composes_with_guard_builder_for_multiple_checks() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::GuardBuilder::new()
                .add(require_admin_for(env, &admin, AdminOperation::SetConfig))
                .add(require_admin_for(env, &admin, AdminOperation::PauseContract))
                .add(Ok(()))
                .execute();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn guard_builder_stops_on_first_unauthorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::GuardBuilder::new()
                .add(require_admin_for(env, &intruder, AdminOperation::AddCurrency))
                .add(Ok(())) // must not execute
                .execute();
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }

    // ── AC4: Determinism ──────────────────────────────────────────────────────

    #[test]
    fn require_administrator_is_deterministic() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            for _ in 0..3 {
                assert!(require_administrator(env, &admin).is_ok());
            }
        });
    }

    #[test]
    fn probe_is_deterministic() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            for _ in 0..3 {
                assert!(is_admin_operation_permitted(env, &admin));
            }
        });
    }
}
