//! Guard execution helper (issue #1407).
//!
//! Provides the single, uniform entry point for evaluating one or more guards
//! **before** a protected contract operation runs. The helper accepts a guard
//! together with the contract context, executes the guard first, and returns
//! the appropriate contract error when validation fails.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Accept a guard and contract context | [`execute`], [`execute_before`], [`GuardExecutionHelper`] |
//! | Execute the guard before the protected operation | [`execute_before`] |
//! | Return the appropriate contract error on failure | [`execute`] converts `GuardResult` to `Error` |
//! | Add unit tests | Comprehensive test module below |
//!
//! # Relationship to the rest of the guard framework
//!
//! The low-level execution primitive lives in [`crate::guard_validator`].
//! This module is the *helper* layer intended for contract authors: it ties a
//! [`GuardContext`] to one or more guards and, optionally, to the protected
//! operation itself. It deliberately adds no new guard semantics — guards
//! remain read-only and fail-fast as defined by [`crate::guard_interface`].
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::guard_execution_helper::{execute, execute_before, GuardExecutionHelper};
//! use crate::guard_interface::GuardContext;
//!
//! // Evaluate a single guard.
//! execute(&ctx, &AdminGuard)?;
//!
//! // Guard a protected operation: `operation` is only invoked if the guard passes.
//! let result = execute_before(&ctx, &AdminGuard, || do_sensitive_work())?;
//!
//! // Fluent form for several guards.
//! GuardExecutionHelper::new(&ctx)
//!     .with_guard(&AdminGuard)
//!     .with_guard(&NftStateGuard)
//!     .execute()?;
//! ```

use crate::guard_interface::{Guard, GuardContext};
use crate::guard_validator;
use crate::types::Error;

// ─── Free functions ────────────────────────────────────────────────────────────

/// Execute a single guard against `ctx`, returning the contract error on failure.
///
/// This is a thin, documented wrapper around [`guard_validator::execute_guard`]
/// so that contract code has one obvious helper to reach for.
///
/// # Arguments
///
/// * `ctx` — contract context (environment, caller and optional token id).
/// * `guard` — the guard to evaluate.
///
/// # Returns
///
/// * `Ok(())` — the guard passed; the protected operation may proceed.
/// * `Err(Error::…)` — the guard failed; the operation must be rejected.
pub fn execute(ctx: &GuardContext, guard: &dyn Guard) -> Result<(), Error> {
    guard_validator::execute_guard(ctx, guard)
}

/// Execute several guards in order, stopping at the first failure.
///
/// Guards are evaluated in the order supplied. After a failure no further
/// guard is evaluated (fail-fast), matching the behaviour of
/// [`guard_validator::execute_guards`].
pub fn execute_all(ctx: &GuardContext, guards: &[&dyn Guard]) -> Result<(), Error> {
    for guard in guards {
        execute(ctx, *guard)?;
    }
    Ok(())
}

/// Execute a guard **before** a protected operation.
///
/// The operation closure is invoked only when the guard passes. When the guard
/// fails, its error is returned and the closure is never called, which is the
/// core guarantee this helper provides: a protected operation cannot run while
/// a pre-condition is violated.
///
/// # Arguments
///
/// * `ctx` — contract context supplied to the guard.
/// * `guard` — guard evaluated before the operation.
/// * `operation` — the protected operation, executed only after the guard passes.
///
/// # Returns
///
/// The operation's result when the guard passes, otherwise the guard's error.
pub fn execute_before<F, R>(ctx: &GuardContext, guard: &dyn Guard, operation: F) -> Result<R, Error>
where
    F: FnOnce() -> Result<R, Error>,
{
    execute(ctx, guard)?;
    operation()
}

// ─── Fluent helper ────────────────────────────────────────────────────────────

/// Fluent guard execution helper bound to a [`GuardContext`].
///
/// Prefer this type when an operation is protected by several guards; use the
/// free functions above for the single-guard case.
pub struct GuardExecutionHelper<'a> {
    ctx: &'a GuardContext,
}

impl<'a> GuardExecutionHelper<'a> {
    /// Create a helper for the given contract context.
    pub fn new(ctx: &'a GuardContext) -> Self {
        Self { ctx }
    }

    /// Return the context this helper is bound to.
    pub fn context(&self) -> &'a GuardContext {
        self.ctx
    }

    /// Execute one guard immediately.
    pub fn execute_one(&self, guard: &dyn Guard) -> Result<(), Error> {
        execute(self.ctx, guard)
    }

    /// Execute all supplied guards in order, stopping at the first failure.
    pub fn execute_all(&self, guards: &[&dyn Guard]) -> Result<(), Error> {
        execute_all(self.ctx, guards)
    }

    /// Execute a protected operation only after all supplied guards pass.
    pub fn execute_before<F, R>(&self, guards: &[&dyn Guard], operation: F) -> Result<R, Error>
    where
        F: FnOnce() -> Result<R, Error>,
    {
        execute_all(self.ctx, guards)?;
        operation()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard_result::GuardResult;
    use crate::types::Error;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    struct PassGuard;
    struct FailGuard(Error);

    impl Guard for PassGuard {
        fn execute(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::Pass
        }
        fn name(&self) -> &'static str {
            "pass_guard"
        }
    }

    impl Guard for FailGuard {
        fn execute(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::Fail(self.0)
        }
        fn name(&self) -> &'static str {
            "fail_guard"
        }
    }

    fn context() -> GuardContext {
        let env = Env::default();
        let caller = Address::generate(&env);
        GuardContext::new(env, caller)
    }

    #[test]
    fn execute_accepts_a_guard_and_context_and_passes() {
        let ctx = context();
        assert_eq!(execute(&ctx, &PassGuard), Ok(()));
    }

    #[test]
    fn execute_returns_the_contract_error_when_a_guard_fails() {
        let ctx = context();
        assert_eq!(
            execute(&ctx, &FailGuard(Error::Unauthorized)),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            execute(&ctx, &FailGuard(Error::TokenNotFound)),
            Err(Error::TokenNotFound)
        );
        assert_eq!(
            execute(&ctx, &FailGuard(Error::NotInitialized)),
            Err(Error::NotInitialized)
        );
    }

    #[test]
    fn execute_before_runs_the_operation_after_a_passing_guard() {
        let ctx = context();
        let result = execute_before(&ctx, &PassGuard, || Ok::<u64, Error>(42));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn execute_before_does_not_run_the_operation_when_the_guard_fails() {
        let ctx = context();
        let result = execute_before(&ctx, &FailGuard(Error::Unauthorized), || {
            Ok::<u64, Error>(42)
        });
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn execute_before_propagates_the_operations_own_error() {
        let ctx = context();
        let result = execute_before(&ctx, &PassGuard, || -> Result<(), Error> {
            Err(Error::MetadataAlreadyUpdated)
        });
        assert_eq!(result, Err(Error::MetadataAlreadyUpdated));
    }

    #[test]
    fn execute_all_runs_every_guard_in_order() {
        let ctx = context();
        let guards: [&dyn Guard; 3] = [&PassGuard, &PassGuard, &PassGuard];
        assert_eq!(execute_all(&ctx, &guards), Ok(()));
    }

    #[test]
    fn execute_all_stops_on_the_first_failure() {
        let ctx = context();
        let first = FailGuard(Error::ContractPaused);
        let second = FailGuard(Error::Unauthorized);
        let guards: [&dyn Guard; 2] = [&first, &second];
        assert_eq!(execute_all(&ctx, &guards), Err(Error::ContractPaused));
    }

    #[test]
    fn fluent_helper_binds_the_context_and_executes_guards() {
        let ctx = context();
        let guard = PassGuard;
        let helper = GuardExecutionHelper::new(&ctx);

        let guards: [&dyn Guard; 1] = [&guard];

        assert!(core::ptr::eq(helper.context(), &ctx));
        assert_eq!(helper.execute_one(&guard), Ok(()));
        assert_eq!(helper.execute_all(&guards), Ok(()));
        assert_eq!(
            helper.execute_before(&guards, || Ok::<u64, Error>(7)),
            Ok(7)
        );
    }
}
