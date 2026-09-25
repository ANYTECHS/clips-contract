//! Guard validator — responsible for executing guards before protected contract
//! operations.
//!
//! This module provides helpers that uniformly execute guards and return
//! appropriate contract errors when validation fails.
//!
//! # Acceptance Criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Accept a guard and contract context | [`execute_guard`], [`execute_guards`] |
//! | Execute guard before protected operation | Single-guard execution |
//! | Return appropriate contract error on failure | Automatic error conversion |
//! | Support batch validation | Multiple guard support |
//! | Add unit tests | Comprehensive test module |
//!
//! # Design
//!
//! The validator provides a unified entry point for guard execution:
//!
//! 1. **Single-guard validation** — [`execute_guard`] executes one guard.
//! 2. **Batch validation** — [`execute_guards`] executes a sequence of guards.
//! 3. **Deterministic order** — guards execute in the order provided.
//! 4. **Fail-fast** — validation stops on the first guard failure.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::guard_interface::{Guard, GuardContext};
//! use crate::guard_validator;
//!
//! // Single guard
//! guard_validator::execute_guard(&ctx, &my_guard)?;
//!
//! // Multiple guards
//! let guards: Vec<Box<dyn Guard>> = vec![
//!     Box::new(admin_guard),
//!     Box::new(pause_guard),
//! ];
//! guard_validator::execute_guards(&ctx, &guards)?;
//! ```

use crate::guard_interface::{Guard, GuardContext};
use crate::guard_result::GuardResult;
use crate::types::Error;

// ─── Single-guard validation ──────────────────────────────────────────────────

/// Execute a single guard and return a contract result.
///
/// # Arguments
///
/// * `ctx` — Guard context with environment, caller, and optional token_id.
/// * `guard` — The guard to execute.
///
/// # Returns
///
/// - `Ok(())` — guard passed; operation may proceed.
/// - `Err(err)` — guard failed; operation must be rejected.
///
/// # Example
///
/// ```rust,ignore
/// guard_validator::execute_guard(&ctx, &my_guard)?;
/// // If Ok, the guard passed and the protected operation can proceed.
/// ```
pub fn execute_guard(ctx: &GuardContext, guard: &dyn Guard) -> Result<(), Error> {
    let result = guard.execute(ctx);
    result.to_contract_result()
}

// ─── Batch-guard validation ───────────────────────────────────────────────────

/// Execute a sequence of guards, stopping on the first failure.
///
/// # Arguments
///
/// * `ctx` — Guard context with environment, caller, and optional token_id.
/// * `guards` — Slice of guards to execute in order.
///
/// # Returns
///
/// - `Ok(())` — all guards passed; operation may proceed.
/// - `Err(err)` — first failing guard's error.
///
/// # Behavior
///
/// Guards are executed in the order they appear in the slice. On the first
/// failure, evaluation stops immediately and the error is returned.
///
/// # Example
///
/// ```rust,ignore
/// let guards: Vec<Box<dyn Guard>> = vec![
///     Box::new(admin_guard),
///     Box::new(pause_guard),
/// ];
/// guard_validator::execute_guards(&ctx, &guards)?;
/// // If Ok, all guards passed and the protected operation can proceed.
/// ```
pub fn execute_guards(ctx: &GuardContext, guards: &[Box<dyn Guard>]) -> Result<(), Error> {
    for guard in guards {
        execute_guard(ctx, guard.as_ref())?;
    }
    Ok(())
}

// ─── Builder for dynamic guard sequences ───────────────────────────────────────

/// Builder for creating and executing a dynamic sequence of guards.
///
/// Provides a fluent interface for composing guards and executing them in
/// deterministic order, stopping on the first failure.
///
/// # Example
///
/// ```rust,ignore
/// use crate::guard_validator::GuardValidator;
///
/// GuardValidator::new(&ctx)
///     .with_guard(&admin_guard)
///     .with_guard(&pause_guard)
///     .with_guard(&token_state_guard)
///     .execute()?;
/// ```
pub struct GuardValidator<'a> {
    ctx: &'a GuardContext,
    guards: alloc::vec::Vec<&'a dyn Guard>,
}

use alloc::vec;

impl<'a> GuardValidator<'a> {
    /// Create a new validator with the given guard context.
    ///
    /// # Arguments
    ///
    /// * `ctx` — Guard context for all guards in this validator.
    pub fn new(ctx: &'a GuardContext) -> Self {
        Self {
            ctx,
            guards: vec![],
        }
    }

    /// Add a guard to the sequence.
    ///
    /// Guards are executed in the order they are added.
    ///
    /// # Arguments
    ///
    /// * `guard` — Guard to add to the sequence.
    pub fn with_guard(mut self, guard: &'a dyn Guard) -> Self {
        self.guards.push(guard);
        self
    }

    /// Add multiple guards to the sequence.
    ///
    /// # Arguments
    ///
    /// * `guards` — Slice of guards to add, in order.
    pub fn with_guards(mut self, guards: &[&'a dyn Guard]) -> Self {
        self.guards.extend_from_slice(guards);
        self
    }

    /// Execute all guards in sequence, stopping on first failure.
    ///
    /// # Returns
    ///
    /// - `Ok(())` — all guards passed.
    /// - `Err(err)` — first failing guard's error.
    pub fn execute(self) -> Result<(), Error> {
        for guard in self.guards {
            execute_guard(self.ctx, guard)?;
        }
        Ok(())
    }

    /// Return the number of guards in this validator.
    pub fn len(&self) -> usize {
        self.guards.len()
    }

    /// Check if this validator has no guards.
    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard_interface::{Guard, GuardContext};
    use crate::types::Error;

    struct PassGuard(&'static str);

    impl Guard for PassGuard {
        fn execute(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::Pass
        }

        fn name(&self) -> &'static str {
            self.0
        }
    }

    struct FailGuard(&'static str, Error);

    impl Guard for FailGuard {
        fn execute(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::Fail(self.1)
        }

        fn name(&self) -> &'static str {
            self.0
        }
    }

    fn make_context() -> GuardContext {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        GuardContext::new(env, caller)
    }

    #[test]
    fn test_execute_guard_pass() {
        let ctx = make_context();
        let guard = PassGuard("test_pass");

        let result = execute_guard(&ctx, &guard);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_execute_guard_fail() {
        let ctx = make_context();
        let guard = FailGuard("test_fail", Error::Unauthorized);

        let result = execute_guard(&ctx, &guard);

        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn test_execute_guards_all_pass() {
        let ctx = make_context();
        let guards: Vec<Box<dyn Guard>> = vec![
            Box::new(PassGuard("guard_1")),
            Box::new(PassGuard("guard_2")),
            Box::new(PassGuard("guard_3")),
        ];

        let result = execute_guards(&ctx, &guards);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_execute_guards_first_fails() {
        let ctx = make_context();
        let guards: Vec<Box<dyn Guard>> = vec![
            Box::new(FailGuard("guard_1", Error::Unauthorized)),
            Box::new(PassGuard("guard_2")),
        ];

        let result = execute_guards(&ctx, &guards);

        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn test_execute_guards_second_fails() {
        let ctx = make_context();
        let guards: Vec<Box<dyn Guard>> = vec![
            Box::new(PassGuard("guard_1")),
            Box::new(FailGuard("guard_2", Error::ContractPaused)),
        ];

        let result = execute_guards(&ctx, &guards);

        assert_eq!(result, Err(Error::ContractPaused));
    }

    #[test]
    fn test_execute_guards_stops_on_first_failure() {
        let ctx = make_context();
        let guards: Vec<Box<dyn Guard>> = vec![
            Box::new(PassGuard("guard_1")),
            Box::new(FailGuard("guard_2", Error::ContractPaused)),
            Box::new(FailGuard("guard_3", Error::Unauthorized)), // Should not be executed
        ];

        let result = execute_guards(&ctx, &guards);

        // Should return the error from guard_2, not guard_3
        assert_eq!(result, Err(Error::ContractPaused));
    }

    #[test]
    fn test_guard_validator_empty() {
        let ctx = make_context();
        let validator = GuardValidator::new(&ctx);

        assert!(validator.is_empty());
        assert_eq!(validator.len(), 0);
        assert_eq!(validator.execute(), Ok(()));
    }

    #[test]
    fn test_guard_validator_single_pass() {
        let ctx = make_context();
        let guard = PassGuard("test_guard");

        let result = GuardValidator::new(&ctx).with_guard(&guard).execute();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_guard_validator_single_fail() {
        let ctx = make_context();
        let guard = FailGuard("test_guard", Error::TokenNotFound);

        let result = GuardValidator::new(&ctx).with_guard(&guard).execute();

        assert_eq!(result, Err(Error::TokenNotFound));
    }

    #[test]
    fn test_guard_validator_multiple_pass() {
        let ctx = make_context();
        let guard1 = PassGuard("guard_1");
        let guard2 = PassGuard("guard_2");
        let guard3 = PassGuard("guard_3");

        let result = GuardValidator::new(&ctx)
            .with_guard(&guard1)
            .with_guard(&guard2)
            .with_guard(&guard3)
            .execute();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_guard_validator_multiple_fail() {
        let ctx = make_context();
        let guard1 = PassGuard("guard_1");
        let guard2 = FailGuard("guard_2", Error::ContractPaused);
        let guard3 = PassGuard("guard_3");

        let result = GuardValidator::new(&ctx)
            .with_guard(&guard1)
            .with_guard(&guard2)
            .with_guard(&guard3)
            .execute();

        assert_eq!(result, Err(Error::ContractPaused));
    }

    #[test]
    fn test_guard_validator_with_guards() {
        let ctx = make_context();
        let guard1 = PassGuard("guard_1");
        let guard2 = PassGuard("guard_2");

        let result = GuardValidator::new(&ctx)
            .with_guards(&[&guard1, &guard2])
            .execute();

        assert_eq!(result, Ok(()));
        assert_eq!(GuardValidator::new(&ctx).with_guards(&[&guard1, &guard2]).len(), 2);
    }

    #[test]
    fn test_guard_validator_stops_on_first_failure() {
        let ctx = make_context();
        let guard1 = PassGuard("guard_1");
        let guard2 = FailGuard("guard_2", Error::TokenNotFound);
        let guard3 = PassGuard("guard_3");

        let result = GuardValidator::new(&ctx)
            .with_guard(&guard1)
            .with_guard(&guard2)
            .with_guard(&guard3)
            .execute();

        assert_eq!(result, Err(Error::TokenNotFound));
    }
}
