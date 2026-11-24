//! Admin authorization guard — restrict operations to administrators (issues #1091, #1410).
//!
//! This guard module re-exports the existing admin access control functionality
//! from [`crate::admin_access_control_guard`] as part of the centralized guards
//! framework, and adds [`AdminGuard`] — the standardized [`Guard`] trait adapter
//! requested by issue #1410.
//!
//! For detailed documentation, see [`crate::admin_access_control_guard`].

use crate::guard_interface::{Guard, GuardContext};
use crate::guard_result::GuardResult;
use crate::types::Error;

pub use crate::admin_access_control_guard::{
    check_caller_is_admin, get_configured_admin, require_admin,
};

// ─── Standardized guard adapter (issue #1410) ─────────────────────────────────

/// Guard that restricts administrative operations to authorized administrators.
///
/// The guard retrieves the configured admin address, compares it with the
/// caller, and rejects unauthorized callers. Registered multi-admins from
/// [`crate::administrator_storage`] are accepted as well, preserving the
/// existing authorization policy.
///
/// # Errors
///
/// - [`Error::NotInitialized`] — no admin has been configured yet.
/// - [`Error::Unauthorized`] — the caller is not an authorized administrator.
///
/// # Usage
///
/// ```rust,ignore
/// use crate::guard_execution_helper;
/// use crate::guards::admin::AdminGuard;
///
/// guard_execution_helper::execute(&ctx, &AdminGuard)?;
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct AdminGuard;

impl Guard for AdminGuard {
    fn execute(&self, ctx: &GuardContext) -> GuardResult {
        // 1. Retrieve the configured admin address.
        let configured = get_configured_admin(&ctx.env);

        // 2. Compare it with the caller.
        if matches!(&configured, Ok(admin) if *admin == ctx.caller) {
            return GuardResult::Pass;
        }

        // Registered multi-admins are equally authorized.
        if check_caller_is_admin(&ctx.env, &ctx.caller) {
            return GuardResult::Pass;
        }

        // 3. Reject unauthorized callers with the appropriate contract error.
        GuardResult::Fail(match configured {
            Err(_) => Error::NotInitialized,
            Ok(_) => Error::Unauthorized,
        })
    }

    fn name(&self) -> &'static str {
        "admin_guard"
    }
}

/// Require admin authorization for the operation described by `ctx`.
///
/// Convenience wrapper around [`AdminGuard`] for contract entry points.
pub fn require_admin_for_context(ctx: &GuardContext) -> Result<(), Error> {
    crate::guard_execution_helper::execute(ctx, &AdminGuard)
}

use crate::guards::{Guard, GuardContext, GuardResult};

/// Admin guard implementing the common [`Guard`] contract.
///
/// The guard authorizes the operation when [`GuardContext::caller`] matches the
/// configured admin stored under [`crate::types::DataKey::Admin`].
pub struct AdminGuard;

impl Guard for AdminGuard {
    fn check(env: &soroban_sdk::Env, ctx: &GuardContext) -> GuardResult {
        match get_configured_admin(env) {
            Some(admin) if admin == ctx.caller => GuardResult::Allow,
            _ => GuardResult::Deny(crate::types::Error::Unauthorized),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::guard_interface::GuardContext;
    use crate::types::{DataKey, Error};
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn require_admin_accepts_configured_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert!(require_admin(env, &admin).is_ok());
        });
    }

    #[test]
    fn require_admin_rejects_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert_eq!(require_admin(env, &other), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn check_caller_is_admin_returns_boolean() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert!(check_caller_is_admin(env, &admin));
            let other = Address::generate(env);
            assert!(!check_caller_is_admin(env, &other));
        });
    }

    #[test]
    fn get_configured_admin_returns_stored_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            let retrieved = get_configured_admin(env).unwrap();
            assert_eq!(retrieved, admin);
        });
    }

    // ── AdminGuard (issue #1410) ─────────────────────────────────────────────

    #[test]
    fn admin_guard_passes_for_configured_admin() {
    #[test]
    fn admin_guard_allows_configured_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            let ctx = GuardContext::new(env.clone(), admin);
            assert_eq!(AdminGuard.execute(&ctx), GuardResult::Pass);
            assert_eq!(require_admin_for_context(&ctx), Ok(()));
            assert_eq!(AdminGuard.name(), "admin_guard");
        });
    }

    #[test]
    fn admin_guard_passes_for_registered_multi_admin() {
        with_contract(|env| {
            let contract_admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            env.storage()
                .instance()
                .set(&DataKey::Admin, &contract_admin);
            administrator_storage::add_admin(env, &multi_admin);

            let ctx = GuardContext::new(env.clone(), multi_admin);
            assert_eq!(AdminGuard.execute(&ctx), GuardResult::Pass);
            let ctx = GuardContext::new(admin.clone());
            assert_eq!(AdminGuard::check(env, &ctx), GuardResult::Allow);
        });
    }

    #[test]
    fn admin_guard_rejects_unauthorized_caller() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            let ctx = GuardContext::new(env.clone(), stranger);
            assert_eq!(
                AdminGuard.execute(&ctx),
                GuardResult::Fail(Error::Unauthorized)
            );
            assert_eq!(require_admin_for_context(&ctx), Err(Error::Unauthorized));
    fn admin_guard_denies_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            let ctx = GuardContext::new(other);
            assert_eq!(
                AdminGuard::check(env, &ctx),
                GuardResult::Deny(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn admin_guard_rejects_uninitialized_contract() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx = GuardContext::new(env.clone(), caller);

            assert_eq!(
                AdminGuard.execute(&ctx),
                GuardResult::Fail(Error::NotInitialized)
    fn admin_guard_denies_when_no_admin_configured() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx = GuardContext::new(caller);
            assert_eq!(
                AdminGuard::check(env, &ctx),
                GuardResult::Deny(Error::Unauthorized)
            );
        });
    }
}
