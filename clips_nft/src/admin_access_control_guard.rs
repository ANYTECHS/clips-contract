//! Admin access control guard (Issue #1090).
//!
//! Implements a guard that restricts administrative contract operations to
//! authorized administrators.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Retrieve configured admin address | [`get_configured_admin`] |
//! | Compare admin with caller | [`check_caller_is_admin`] |
//! | Reject unauthorized callers | [`require_admin`] |
//! | Add admin authorization tests | Tests module with comprehensive coverage |
//!
//! # Guard priority
//!
//! This guard checks admin status in priority order:
//!
//! 1. **Registered multi-admin** — addresses stored in [`crate::administrator_storage`].
//! 2. **Contract-level admin** — the address stored at [`DataKey::Admin`].
//!
//! The first match wins; checking stops on success.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full guard with auth requirement
//! admin_access_control_guard::require_admin(&env, &caller)?;
//!
//! // Check admin status without auth
//! if admin_access_control_guard::check_caller_is_admin(&env, &caller) {
//!     // caller is an authorized admin
//! }
//!
//! // Get the primary admin
//! let admin = admin_access_control_guard::get_configured_admin(&env)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::types::{DataKey, Error};

// ─── Primary integration point ────────────────────────────────────────────────

/// Require that `caller` is an authorized administrator.
///
/// Verifies admin status and demands an authorization signature from the caller
/// via [`Address::require_auth`]. If the caller is not an admin, returns
/// [`Error::Unauthorized`]. If the contract has not been initialized (no admin
/// stored), returns [`Error::NotInitialized`].
///
/// A caller is an authorized admin if **any** of the following holds:
///
/// 1. The caller is in the registered multi-admin list
///    (via [`crate::administrator_storage`]).
/// 2. The caller matches the contract-level admin stored at [`DataKey::Admin`].
///
/// This is the authoritative guard for admin operations. Use it whenever you
/// need to verify both admin status **and** authorize a sensitive operation.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
/// - [`Error::Unauthorized`] — caller is not an authorized admin.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    // Verify contract is initialized
    if !is_initialized(env) {
        return Err(Error::NotInitialized);
    }

    // Demand a signed authorization envelope from the caller
    caller.require_auth();

    if check_caller_is_admin(env, caller) {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

// ─── Individual authorization probes ─────────────────────────────────────────

/// Return `true` when `caller` is an authorized administrator.
///
/// Performs no authorization call and no error handling—purely a read-only
/// check. Use this when you already have the required authorization or need
/// to make a non-authoritative probe.
///
/// A caller is an admin if **any** of the following holds:
///
/// 1. The caller is in the registered multi-admin list
///    (via [`crate::administrator_storage::is_admin`]).
/// 2. The caller matches the contract-level admin at [`DataKey::Admin`].
///
/// Returns `false` if the contract is not yet initialized.
///
/// # Example
///
/// ```rust,ignore
/// if admin_access_control_guard::check_caller_is_admin(&env, &caller) {
///     // caller is an authorized admin; safe to proceed
/// }
/// ```
pub fn check_caller_is_admin(env: &Env, caller: &Address) -> bool {
    // Priority 1: Check registered multi-admin list
    if administrator_storage::is_admin(env, caller) {
        return true;
    }

    // Priority 2: Check contract-level admin
    if let Some(admin) = env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
    {
        if *caller == admin {
            return true;
        }
    }

    false
}

/// Retrieve the contract-level admin address.
///
/// Returns the primary admin address stored at [`DataKey::Admin`], or
/// `NotInitialized` if the contract has not been initialized.
///
/// This returns only the contract-level admin, not members of the registered
/// multi-admin list. For checking whether a specific caller is authorized as
/// an admin, use [`check_caller_is_admin`] or [`require_admin`] instead.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has no admin configured.
///
/// # Example
///
/// ```rust,ignore
/// let admin = admin_access_control_guard::get_configured_admin(&env)?;
/// if *caller == admin {
///     // caller is the primary admin
/// }
/// ```
pub fn get_configured_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

/// Return `true` when the contract has been initialized (admin is set).
///
/// This is a non-authoritative probe with no side effects.
fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ───────────────────────────────────────────────────────

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_contract_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    // ── require_admin ──────────────────────────────────────────────────────

    #[test]
    fn require_admin_accepts_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_contract_admin(env, &admin);

            assert!(require_admin(env, &admin).is_ok());
        });
    }

    #[test]
    fn require_admin_accepts_registered_multi_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_contract_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(require_admin(env, &multi_admin).is_ok());
        });
    }

    #[test]
    fn require_admin_rejects_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            setup_contract_admin(env, &admin);

            assert_eq!(require_admin(env, &other), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn require_admin_rejects_when_contract_not_initialized() {
        with_contract(|env| {
            let caller = Address::generate(env);

            assert_eq!(require_admin(env, &caller), Err(Error::NotInitialized));
        });
    }

    // ── check_caller_is_admin ──────────────────────────────────────────────

    #[test]
    fn check_caller_is_admin_returns_true_for_contract_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_contract_admin(env, &admin);

            assert!(check_caller_is_admin(env, &admin));
        });
    }

    #[test]
    fn check_caller_is_admin_returns_true_for_multi_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_contract_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(check_caller_is_admin(env, &multi_admin));
        });
    }

    #[test]
    fn check_caller_is_admin_returns_false_for_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            setup_contract_admin(env, &admin);

            assert!(!check_caller_is_admin(env, &other));
        });
    }

    #[test]
    fn check_caller_is_admin_returns_false_when_not_initialized() {
        with_contract(|env| {
            let caller = Address::generate(env);

            assert!(!check_caller_is_admin(env, &caller));
        });
    }

    // ── get_configured_admin ───────────────────────────────────────────────

    #[test]
    fn get_configured_admin_returns_the_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_contract_admin(env, &admin);

            let retrieved = get_configured_admin(env).unwrap();
            assert_eq!(retrieved, admin);
        });
    }

    #[test]
    fn get_configured_admin_returns_not_initialized_when_no_admin() {
        with_contract(|env| {
            assert_eq!(
                get_configured_admin(env),
                Err(Error::NotInitialized)
            );
        });
    }

    // ── Priority order verification ────────────────────────────────────────

    #[test]
    fn multi_admin_is_accepted_even_with_different_contract_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_contract_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(check_caller_is_admin(env, &multi_admin));
            assert!(check_caller_is_admin(env, &contract_admin));
        });
    }

    #[test]
    fn multiple_multi_admins_are_all_accepted() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin_1 = Address::generate(env);
            let multi_admin_2 = Address::generate(env);

            setup_contract_admin(env, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin_1);
            administrator_storage::add_admin(env, &multi_admin_2);

            assert!(check_caller_is_admin(env, &multi_admin_1));
            assert!(check_caller_is_admin(env, &multi_admin_2));
            assert!(check_caller_is_admin(env, &contract_admin));
        });
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn admin_check_is_deterministic_across_multiple_calls() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_contract_admin(env, &admin);

            // Call multiple times; result should be consistent
            assert!(check_caller_is_admin(env, &admin));
            assert!(check_caller_is_admin(env, &admin));
            assert!(check_caller_is_admin(env, &admin));
        });
    }
}
