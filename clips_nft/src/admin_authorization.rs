//! Admin authorization guard (issue #1253).
//!
//! Restricts administrative operations to the configured contract administrator.
//! This module is the canonical entry point for admin authorization across all
//! contract operations that require administrative privileges.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Retrieve the configured admin | [`get_admin`] |
//! | Verify the caller's identity | [`verify_admin_caller`] |
//! | Reject unauthorized administrative calls | [`require_admin_authorization`] |
//! | Add admin authorization tests | Tests module with comprehensive coverage |
//!
//! # Design
//!
//! Three focused functions covering the full authorization lifecycle:
//!
//! | Function | Purpose |
//! |----------|---------|
//! | [`get_admin`] | Retrieve the primary admin address from storage |
//! | [`verify_admin_caller`] | Check whether the caller is an authorized admin (no auth demand) |
//! | [`require_admin_authorization`] | Full guard: retrieve → verify → require auth signature |
//!
//! # Admin resolution order
//!
//! Admin checks follow a priority order to support the contract's two-tier
//! admin model:
//!
//! 1. **Registered multi-admin list** — checked via [`crate::administrator_storage`].
//! 2. **Contract-level admin** — the single address stored at [`DataKey::Admin`].
//!
//! The first match wins.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Protect an admin-only entry point
//! admin_authorization::require_admin_authorization(&env, &caller)?;
//!
//! // Non-authoritative check (e.g. routing logic)
//! if admin_authorization::verify_admin_caller(&env, &caller) {
//!     // safe to proceed without requiring a signed auth envelope
//! }
//!
//! // Retrieve the primary admin address
//! let admin = admin_authorization::get_admin(&env)?;
//! ```
//!
//! # Errors
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::NotInitialized`] | No admin has been configured yet. |
//! | [`Error::Unauthorized`] | Caller is not the configured admin or a registered multi-admin. |

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::types::{DataKey, Error};

// ─── Retrieval ────────────────────────────────────────────────────────────────

/// Retrieve the primary contract admin address.
///
/// Returns the address stored at [`DataKey::Admin`] (the contract-level admin
/// set during initialization). This does **not** enumerate the multi-admin
/// registry — for membership checks on all admins use [`verify_admin_caller`].
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
///
/// # Example
///
/// ```rust,ignore
/// let admin = admin_authorization::get_admin(&env)?;
/// // admin is the primary contract-level administrator
/// ```
pub fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

// ─── Identity verification ────────────────────────────────────────────────────

/// Verify whether `caller` is an authorized administrator.
///
/// Performs a read-only check with **no** authorization side effects — does
/// not call `require_auth`. Use this for non-authoritative probes such as
/// routing decisions or UI state building.
///
/// A caller is considered an authorized admin when **any** of the following
/// holds (checked in priority order):
///
/// 1. `caller` appears in the registered multi-admin list
///    (via [`crate::administrator_storage::is_admin`]).
/// 2. `caller` matches the contract-level admin at [`DataKey::Admin`].
///
/// Returns `false` when the contract is uninitialized.
///
/// # Example
///
/// ```rust,ignore
/// if admin_authorization::verify_admin_caller(&env, &caller) {
///     // caller holds admin privileges — proceed without auth envelope
/// }
/// ```
pub fn verify_admin_caller(env: &Env, caller: &Address) -> bool {
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

// ─── Authorization guard ──────────────────────────────────────────────────────

/// Require that `caller` is an authorized administrator.
///
/// This is the canonical guard for all administrative entry points. It:
///
/// 1. Verifies the contract has been initialized (admin exists).
/// 2. Demands a signed authorization envelope from `caller`
///    via [`Address::require_auth`].
/// 3. Checks that `caller` is either the contract-level admin or a member
///    of the multi-admin registry.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
/// - [`Error::Unauthorized`] — caller is not an authorized admin.
///
/// # Example
///
/// ```rust,ignore
/// pub fn admin_only_operation(env: Env, caller: Address) -> Result<(), Error> {
///     admin_authorization::require_admin_authorization(&env, &caller)?;
///     // proceed with privileged logic
///     Ok(())
/// }
/// ```
pub fn require_admin_authorization(env: &Env, caller: &Address) -> Result<(), Error> {
    // AC1: Ensure the contract has been initialized
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }

    // AC2: Demand signed authorization from caller
    caller.require_auth();

    // AC3: Reject unauthorized callers
    if verify_admin_caller(env, caller) {
        return Ok(());
    }

    Err(Error::Unauthorized)
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

    // ── AC1: get_admin — retrieve the configured admin ────────────────────────

    #[test]
    fn get_admin_returns_configured_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let retrieved = get_admin(env).expect("admin should be set");
            assert_eq!(retrieved, admin);
        });
    }

    #[test]
    fn get_admin_returns_not_initialized_when_no_admin_set() {
        with_contract(|env| {
            assert_eq!(get_admin(env), Err(Error::NotInitialized));
        });
    }

    #[test]
    fn get_admin_returns_most_recently_set_admin() {
        with_contract(|env| {
            let first_admin = Address::generate(env);
            let second_admin = Address::generate(env);

            setup_admin(env, &first_admin);
            setup_admin(env, &second_admin);

            let retrieved = get_admin(env).expect("admin should be set");
            assert_eq!(retrieved, second_admin);
        });
    }

    // ── AC2: verify_admin_caller — verify the caller's identity ───────────────

    #[test]
    fn verify_admin_caller_returns_true_for_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            assert!(verify_admin_caller(env, &admin));
        });
    }

    #[test]
    fn verify_admin_caller_returns_true_for_registered_multi_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(verify_admin_caller(env, &multi_admin));
        });
    }

    #[test]
    fn verify_admin_caller_returns_false_for_unknown_address() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);

            assert!(!verify_admin_caller(env, &stranger));
        });
    }

    #[test]
    fn verify_admin_caller_returns_false_when_not_initialized() {
        with_contract(|env| {
            let caller = Address::generate(env);

            assert!(!verify_admin_caller(env, &caller));
        });
    }

    #[test]
    fn verify_admin_caller_accepts_both_contract_admin_and_multi_admins() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin_a = Address::generate(env);
            let multi_admin_b = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin_a);
            administrator_storage::add_admin(env, &multi_admin_b);

            assert!(verify_admin_caller(env, &contract_admin));
            assert!(verify_admin_caller(env, &multi_admin_a));
            assert!(verify_admin_caller(env, &multi_admin_b));
        });
    }

    #[test]
    fn verify_admin_caller_returns_false_after_multi_admin_removed() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(verify_admin_caller(env, &multi_admin));

            administrator_storage::remove_admin(env, &multi_admin);
            assert!(!verify_admin_caller(env, &multi_admin));
        });
    }

    #[test]
    fn verify_admin_caller_is_deterministic() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            for _ in 0..3 {
                assert!(verify_admin_caller(env, &admin));
            }
        });
    }

    // ── AC3: require_admin_authorization — reject unauthorized callers ─────────

    #[test]
    fn require_admin_authorization_accepts_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            assert!(require_admin_authorization(env, &admin).is_ok());
        });
    }

    #[test]
    fn require_admin_authorization_accepts_registered_multi_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(require_admin_authorization(env, &multi_admin).is_ok());
        });
    }

    #[test]
    fn require_admin_authorization_rejects_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);

            assert_eq!(
                require_admin_authorization(env, &intruder),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_admin_authorization_rejects_when_uninitialized() {
        with_contract(|env| {
            let caller = Address::generate(env);

            assert_eq!(
                require_admin_authorization(env, &caller),
                Err(Error::NotInitialized)
            );
        });
    }

    #[test]
    fn require_admin_authorization_returns_not_initialized_before_unauthorized() {
        // NotInitialized must be returned when no admin is set, even if
        // the caller might happen to match a multi-admin entry.
        with_contract(|env| {
            let caller = Address::generate(env);
            // Register as multi-admin but never set the contract-level admin
            administrator_storage::add_admin(env, &caller);

            // DataKey::Admin is absent → must be NotInitialized
            assert_eq!(
                require_admin_authorization(env, &caller),
                Err(Error::NotInitialized)
            );
        });
    }

    #[test]
    fn require_admin_authorization_rejects_after_multi_admin_removed() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            // Was authorized
            assert!(require_admin_authorization(env, &multi_admin).is_ok());

            // After removal, should be rejected
            administrator_storage::remove_admin(env, &multi_admin);
            assert_eq!(
                require_admin_authorization(env, &multi_admin),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_admin_authorization_accepts_multiple_multi_admins() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin_1 = Address::generate(env);
            let multi_admin_2 = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin_1);
            administrator_storage::add_admin(env, &multi_admin_2);

            assert!(require_admin_authorization(env, &contract_admin).is_ok());
            assert!(require_admin_authorization(env, &multi_admin_1).is_ok());
            assert!(require_admin_authorization(env, &multi_admin_2).is_ok());
        });
    }

    // ── Priority order verification ───────────────────────────────────────────

    #[test]
    fn multi_admin_checked_before_contract_level_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            // Both should pass; multi-admin registry takes priority
            assert!(verify_admin_caller(env, &multi_admin));
            assert!(verify_admin_caller(env, &contract_admin));
        });
    }

    // ── Integration: guard composition ────────────────────────────────────────

    #[test]
    fn admin_authorization_composes_with_sequence_guard() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                require_admin_authorization(env, &admin),
                Ok(()), // a second downstream guard that passes
            );
            assert!(result.is_ok());
        });
    }

    #[test]
    fn admin_authorization_composes_with_guard_builder() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::GuardBuilder::new()
                .add(require_admin_authorization(env, &admin))
                .add(Ok(()))
                .execute();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn admin_authorization_failure_stops_guard_composition() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let intruder = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                require_admin_authorization(env, &intruder),
                Ok(()), // should never execute
            );
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }
}
