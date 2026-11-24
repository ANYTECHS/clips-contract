//! Centralized guard module for contract authorization and validation (issue #1091).
//!
//! This module is the single source of truth for all guard implementations used
//! throughout the smart contract. Guards are reusable authorization and validation
//! checks that:
//!
//! 1. **Authenticate** — verify caller identity and authorization
//! 2. **Validate** — ensure state and parameters are sound
//! 3. **Enforce** — return standardized errors on violation
//!
//! # Guard organization
//!
//! Guards are organized by responsibility domain:
//!
//! ## Authorization guards
//! - [`admin`] — restrict operations to contract administrators
//! - [`owner`] — restrict operations to NFT owners
//! - [`operator`] — restrict operations to authorized operators
//!
//! ## State guards
//! - [`royalty`] — validate royalty configuration state
//! - [`payment`] — detect duplicate or replayed payments
//!
//! ## Composition
//! - [`compose`] — combine multiple guards into sequential checks
//!
//! # Common guard interface
//!
//! All guard implementations follow a shared contract defined by the [`Guard`]
//! trait (issue #1405). A guard receives a [`GuardContext`] carrying the
//! contract context required for evaluation (environment, caller, token id,
//! and operation) and returns a [`GuardResult`]. Guards are composable: any
//! number of guards can be evaluated in sequence, short-circuiting on the
//! first failure.
//!
//! ```rust,ignore
//! use crate::guards::{Guard, GuardContext, TokenFrozenGuard};
//!
//! let ctx = GuardContext::new(&env, caller, token_id, Operation::Transfer);
//! TokenFrozenGuard.check(&ctx)?;
//! ```
//!
//! # Usage patterns
//!
//! ## Single guard
//!
//! ```rust,ignore
//! // Require admin authorization
//! admin::require_admin_auth(&env, &caller)?;
//!
//! // Check owner without authorization demand
//! if owner::is_owner(&env, token_id, &caller) {
//!     // caller owns the token
//! }
//! ```
//!
//! ## Guard composition
//!
//! ```rust,ignore
//! // Combine multiple checks: admin + royalty state validation
//! admin::require_admin_auth(&env, &caller)?;
//! royalty::validate_royalty_state(&env, token_id)?;
//! royalty::validate_recipient(&env, &new_recipient)?;
//! // Safe to proceed with update
//! ```
//!
//! # Guard priority and short-circuit behavior
//!
//! Guards are designed to fail fast:
//!
//! - If the first guard fails, execution stops.
//! - Subsequent guards are not evaluated.
//! - This prevents information leakage and unnecessary computation.
//!
//! # Guard traits and reusable helpers
//!
//! Beyond the domain modules, this module defines a small, uniform surface for
//! guards so they can be reused and composed across contract functions:
//!
//! - [`Guard`] — a trait implemented by every guard, exposing a single
//!   `check` entry point that returns a [`GuardResult`].
//! - [`GuardFn`] — a lightweight adapter that turns any `Fn(&Env) -> GuardResult<()>`
//!   closure into a [`Guard`], enabling ad-hoc guards without new modules.
//! - [`require_all`] — run a sequence of guards, failing fast on the first
//!   violation (short-circuit semantics).
//! - [`require_any`] — succeed if at least one guard passes.
//!
//! These helpers wrap the existing `guard_interface`, `guard_result`, and
//! `guard_composition` infrastructure so callers have one canonical place to
//! reach for authorization and validation checks.
//!
//! # Adding new guards
//!
//! To add a new guard domain:
//!
//! 1. Create a new module in this folder (e.g., `new_domain.rs`).
//! 2. Export from `mod.rs`.
//! 3. Document the guard's responsibility, error conditions, and usage.
//! 4. Add comprehensive tests.
//! 5. Update this module-level documentation.

pub mod admin;
pub mod compose;
pub mod config;
pub mod listing_authorization;
pub mod owner;
pub mod payment;
pub mod purchase_state;
pub mod royalty;
pub mod token_frozen;
pub mod token_active;

pub use admin::{check_caller_is_admin, get_configured_admin, require_admin};
pub use compose::{GuardBuilder, GuardComposition};
pub use config::{
    guard_config_update, is_config_authorized, require_config_admin, require_config_owner,
    validate_config as validate_config_guard,
};
pub use listing_authorization::{
    check_listing_authorization, require_listing_authorization,
};
pub use owner::{check_caller_is_owner, get_owner_for_token, require_owner};
pub use payment::{
    detect_replay_payment, is_payment_already_processed, record_payment_processed,
};
pub use purchase_state::{check_purchase_state, require_purchase_state};
pub use royalty::{
    validate_royalty_recipient, validate_royalty_state, validate_royalty_within_maximum,
};
pub use token_frozen::{require_not_frozen, TokenFrozenGuard};
pub use token_active::{check_token_active, require_token_active};

/// Standardized guard result type.
///
/// Re-exported from [`crate::guard_result`] so all guards share a single
/// success/failure representation that integrates with contract errors.
pub use crate::guard_result::GuardResult;

/// A reusable authorization or validation guard.
///
/// Implementors expose a single `check` entry point so guards can be invoked
/// uniformly and composed via [`require_all`] / [`require_any`].
pub trait Guard {
    /// Evaluate the guard, returning `Ok(())` when satisfied or a
    /// [`crate::types::Error`] describing the violation.
    fn check(&self, env: &soroban_sdk::Env) -> GuardResult<()>;
}

/// Adapter that turns a closure into a [`Guard`].
///
/// This allows ad-hoc guards to be defined inline without creating a new
/// module, while still participating in composition.
pub struct GuardFn<F>(pub F);

impl<F> Guard for GuardFn<F>
where
    F: Fn(&soroban_sdk::Env) -> GuardResult<()>,
{
    fn check(&self, env: &soroban_sdk::Env) -> GuardResult<()> {
        (self.0)(env)
    }
}

/// Run a sequence of guards, failing fast on the first violation.
///
/// Guards are evaluated in order; as soon as one returns an error, evaluation
/// stops and that error is returned. This preserves the short-circuit
/// behaviour documented above.
pub fn require_all(env: &soroban_sdk::Env, guards: &[&dyn Guard]) -> GuardResult<()> {
    for guard in guards {
        guard.check(env)?;
    }
    Ok(())
}

/// Succeed if at least one guard passes.
///
/// Returns the error from the last guard evaluated when none succeed.
pub fn require_any(env: &soroban_sdk::Env, guards: &[&dyn Guard]) -> GuardResult<()> {
    let mut last_error: Option<crate::types::Error> = None;
    for guard in guards {
        match guard.check(env) {
            Ok(()) => return Ok(()),
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or(crate::types::Error::Unauthorized))
}

/// Operation being guarded, used by guards to decide whether an
/// administrative override applies (issue #1416).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Transfer of token ownership.
    Transfer,
    /// Approval of an operator for a token.
    Approve,
    /// Burning of a token.
    Burn,
    /// Administrative operation (e.g. freeze/unfreeze, admin transfer).
    Admin,
}

/// Contract context passed to every guard implementation (issue #1405).
///
/// Carries the environment plus the caller, token id, and operation under
/// evaluation so guards can make consistent authorization decisions.
pub struct GuardContext<'a> {
    /// Contract environment.
    pub env: &'a soroban_sdk::Env,
    /// Address of the caller performing the operation.
    pub caller: soroban_sdk::Address,
    /// Token id the operation targets.
    pub token_id: u32,
    /// Operation being performed.
    pub operation: Operation,
}

impl<'a> GuardContext<'a> {
    /// Construct a new guard context.
    pub fn new(
        env: &'a soroban_sdk::Env,
        caller: soroban_sdk::Address,
        token_id: u32,
        operation: Operation,
    ) -> Self {
        Self {
            env,
            caller,
            token_id,
            operation,
        }
    }
}

/// Common interface all guard implementations must follow (issue #1405).
///
/// Implementors validate the supplied [`GuardContext`] and return
/// `Ok(())` when the operation is permitted, or a standardized
/// [`crate::types::Error`] otherwise. Guards must not mutate state.
pub trait Guard {
    /// Evaluate the guard against the given context.
    fn check(&self, ctx: &GuardContext) -> GuardResult<()>;
}

/// Evaluate a sequence of guards in order, short-circuiting on the first
/// failure. Supports reusable guard composition.
pub fn check_all(guards: &[&dyn Guard], ctx: &GuardContext) -> GuardResult<()> {
    for guard in guards {
        guard.check(ctx)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn guards_module_exports_all_domains() {
        // Compile-time check: all public functions must be re-exported.
        // If this compiles, all public guard functions are accessible.
        let _ = check_caller_is_admin;
        let _ = require_admin;
        let _ = check_caller_is_owner;
        let _ = require_owner;
        let _ = validate_royalty_state;
        let _ = detect_replay_payment;
    }

    #[test]
    fn guard_fn_allows_when_closure_succeeds() {
        let env = Env::default();
        let guard = GuardFn(|_env: &Env| Ok(()));
        assert!(guard.check(&env).is_ok());
    }

    #[test]
    fn guard_fn_denies_when_closure_fails() {
        let env = Env::default();
        let guard = GuardFn(|_env: &Env| Err(crate::types::Error::Unauthorized));
        assert_eq!(guard.check(&env), Err(crate::types::Error::Unauthorized));
    }

    #[test]
    fn require_all_short_circuits_on_first_failure() {
        let env = Env::default();
        let allow = GuardFn(|_env: &Env| Ok(()));
        let deny = GuardFn(|_env: &Env| Err(crate::types::Error::Unauthorized));

        assert!(require_all(&env, &[&allow, &allow]).is_ok());
        assert_eq!(
            require_all(&env, &[&allow, &deny, &allow]),
            Err(crate::types::Error::Unauthorized)
        );
    }

    #[test]
    fn require_any_succeeds_if_one_guard_passes() {
        let env = Env::default();
        let allow = GuardFn(|_env: &Env| Ok(()));
        let deny = GuardFn(|_env: &Env| Err(crate::types::Error::Unauthorized));

        assert!(require_any(&env, &[&deny, &allow]).is_ok());
        assert_eq!(
            require_any(&env, &[&deny, &deny]),
            Err(crate::types::Error::Unauthorized)
        );
    }

    use soroban_sdk::{testutils::Address as _, Address, Env};

    struct AlwaysOk;
    impl Guard for AlwaysOk {
        fn check(&self, _ctx: &GuardContext) -> GuardResult<()> {
            Ok(())
        }
    }

    struct AlwaysFail;
    impl Guard for AlwaysFail {
        fn check(&self, _ctx: &GuardContext) -> GuardResult<()> {
            Err(crate::types::Error::Unauthorized)
        }
    }

    #[test]
    fn guard_context_carries_required_fields() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = GuardContext::new(&env, caller.clone(), 7, Operation::Transfer);
        assert_eq!(ctx.caller, caller);
        assert_eq!(ctx.token_id, 7);
        assert_eq!(ctx.operation, Operation::Transfer);
    }

    #[test]
    fn check_all_passes_when_all_guards_pass() {
        let env = Env::default();
        let ctx = GuardContext::new(&env, Address::generate(&env), 1, Operation::Transfer);
        let guards: [&dyn Guard; 2] = [&AlwaysOk, &AlwaysOk];
        assert!(check_all(&guards, &ctx).is_ok());
    }

    #[test]
    fn check_all_short_circuits_on_failure() {
        let env = Env::default();
        let ctx = GuardContext::new(&env, Address::generate(&env), 1, Operation::Transfer);
        let guards: [&dyn Guard; 2] = [&AlwaysFail, &AlwaysOk];
        assert!(check_all(&guards, &ctx).is_err());
    }

    #[test]
    fn check_all_with_no_guards_succeeds() {
        let env = Env::default();
        let ctx = GuardContext::new(&env, Address::generate(&env), 1, Operation::Transfer);
        let guards: [&dyn Guard; 0] = [];
        assert!(check_all(&guards, &ctx).is_ok());
    }
}
