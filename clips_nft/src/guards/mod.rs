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
pub mod owner;
pub mod payment;
pub mod royalty;
pub mod token_frozen;

pub use admin::{check_caller_is_admin, get_configured_admin, require_admin};
pub use compose::{GuardBuilder, GuardComposition};
pub use owner::{check_caller_is_owner, get_owner_for_token, require_owner};
pub use payment::{
    detect_replay_payment, is_payment_already_processed, record_payment_processed,
};
pub use royalty::{
    validate_royalty_recipient, validate_royalty_state, validate_royalty_within_maximum,
};
pub use token_frozen::{require_not_frozen, TokenFrozenGuard};

/// Convenient type alias for guard results.
pub type GuardResult<T> = Result<T, crate::types::Error>;

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

    #[test]
    fn guards_module_exports_all_domains() {
        // Compile-time check: all public functions must be re-exported.
        // If this compiles, all public guard functions are accessible.
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
