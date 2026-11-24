//! Guard composition framework — combine multiple guards into sequences (issue #1091).
//!
//! This module provides utilities for composing multiple guards into cohesive,
//! reusable authorization and validation sequences.
//!
//! # Motivation
//!
//! Complex contract operations often require multiple validation checks:
//! - Admin must be authorized
//! - Royalty state must be sound
//! - Recipient must be valid
//!
//! Rather than writing ad-hoc sequences, composition frameworks enable:
//! - **Reusability** — guard sequences are defined once and reused
//! - **Clarity** — intent is explicit in the sequence definition
//! - **Testability** — each guard is tested independently and in composition
//! - **Maintainability** — adding/removing guards is localized
//!
//! # Usage
//!
//! ## Builder pattern
//!
//! ```rust,ignore
//! use crate::guards::GuardBuilder;
//!
//! GuardBuilder::new()
//!     .require_admin(&env, &caller)?
//!     .validate_royalty_state(&env, token_id)?
//!     .validate_royalty_recipient(&env, &recipient)?
//!     .build();
//! ```
//!
//! ## Explicit composition
//!
//! ```rust,ignore
//! use crate::guards::{GuardComposition, admin, royalty};
//!
//! GuardComposition::sequence(&env)
//!     .then(|env| admin::require_admin(env, &caller))
//!     .then(|env| royalty::validate_royalty_state(env, token_id))
//!     .then(|env| royalty::validate_royalty_recipient(env, &recipient))
//!     .execute()?;
//! ```
//!
//! # Short-circuit semantics
//!
//! Guard sequences fail fast: if any guard fails, execution stops and the error
//! is immediately returned. This prevents:
//! - Information leakage through timing
//! - Unnecessary computation after a failed check
//! - Inconsistent state from partial guard execution

use soroban_sdk::Env;

use crate::guards::{Guard, GuardContext};
use crate::types::Error;

/// A guard composition that executes checks in sequence (issue #1091).
///
/// If any guard fails, the sequence stops and the error is returned.
/// No state is modified on failure.
///
/// # Example
///
/// ```rust,ignore
/// let result = GuardComposition::sequence(&env)
///     .then(|e| check_admin(e, &caller))
///     .then(|e| check_state(e, token_id))
///     .execute()?;
/// ```
pub struct GuardComposition<'a> {
    env: &'a Env,
    failed: bool,
    /// First error encountered, returned by execute (#1408).
    first_error: Option<Error>,
    error: Option<Error>,
}

impl<'a> GuardComposition<'a> {
    /// Create a new guard composition sequence.
    pub fn sequence(env: &'a Env) -> Self {
        GuardComposition {
            env,
            failed: false,
            first_error: None,
            error: None,
        }
    }

    /// Execute a guard function and continue the sequence.
    ///
    /// If the guard returns an error, the sequence short-circuits and
    /// subsequent guards are skipped.
    pub fn then<F>(mut self, guard: F) -> Self
    where
        F: FnOnce(&'a Env) -> Result<(), Error>,
    {
        if !self.failed {
            if let Err(error) = guard(self.env) {
                self.failed = true;
                self.first_error = Some(error);
            if let Err(e) = guard(self.env) {
                self.failed = true;
                self.error = Some(e);
            }
        }
        self
    }

    /// Execute a [`Guard`] implementation and continue the sequence.
    ///
    /// The guard receives the shared [`GuardContext`] and, if it fails, the
    /// sequence short-circuits and subsequent guards are skipped.
    pub fn then_guard<G>(mut self, guard: &G, ctx: &GuardContext) -> Self
    where
        G: Guard,
    {
        if !self.failed {
            if let Err(e) = guard.check(self.env, ctx) {
                self.failed = true;
                self.error = Some(e);
            }
        }
        self
    }

    /// Complete the sequence and return the result.
    ///
    /// Returns `Ok` if all guards passed, or the first error encountered.
    pub fn execute(self) -> Result<(), Error> {
        match self.first_error {
            Some(error) => Err(error),
        match self.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// A builder for fluent guard composition (issue #1091).
///
/// Provides a builder-style API for defining and executing guard sequences.
/// Guards are stored as boxed [`Guard`] trait objects so arbitrary guard
/// implementations can be composed together.
///
/// # Example
///
/// ```rust,ignore
/// GuardBuilder::with(&env)
///     .guard(&AdminGuard)
///     .guard(&TokenFrozenGuard { token_id })
///     .run(&ctx)?;
/// ```
pub struct GuardBuilder<'a> {
    env: &'a Env,
    guards: soroban_sdk::Vec<&'a dyn Guard>,
}

impl<'a> GuardBuilder<'a> {
    /// Create a new guard builder.
    pub fn new() -> Self {
        panic!("Builder requires an Env reference; use GuardBuilder::with(&env)")
    }

    /// Create a builder bound to an environment.
    pub fn with(env: &'a Env) -> Self {
        GuardBuilder {
            env,
            guards: soroban_sdk::Vec::new(env),
        }
    }

    /// Append a guard to the sequence.
    pub fn guard(mut self, guard: &'a dyn Guard) -> Self {
        self.guards.push_back(guard);
        self
    }

    /// Execute all guards in order with short-circuit semantics.
    ///
    /// Returns `Ok` if every guard passes, or the first error encountered.
    pub fn run(self, ctx: &GuardContext) -> Result<(), Error> {
        for guard in self.guards.iter() {
            guard.check(self.env, ctx)?;
        }
        Ok(())
    }
}

impl<'a> Default for GuardBuilder<'a> {
    fn default() -> Self {
        panic!("Builder requires an Env reference; use GuardBuilder::with(&env)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

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
    fn guard_composition_short_circuits_on_error() {
        with_contract(|env| {
            let result = GuardComposition::sequence(env)
                .then(|_| Err(Error::Unauthorized))
                .then(|_| panic!("should not be called"))
                .execute();

            assert!(result.is_err());
        });
    }

    #[test]
    fn guard_composition_continues_on_success() {
        with_contract(|env| {
            let mut call_count = 0;
            let result = GuardComposition::sequence(env)
                .then(|_| {
                    call_count += 1;
                    Ok(())
                })
                .then(|_| {
                    call_count += 1;
                    Ok(())
                })
                .execute();

            assert!(result.is_ok());
            assert_eq!(call_count, 2);
        });
    }

    #[test]
    fn guard_builder_can_be_created_with_env() {
        with_contract(|env| {
            let _builder = GuardBuilder::with(env);
            // Compile-time check: builder should be constructible with env
        });
    }
    #[test]
    fn composition_runs_in_order_and_reports_first_error() {
        with_contract(|env| {
            let order = soroban_sdk::Vec::<u32>::new(env);
            let result = GuardComposition::sequence(env)
                .then(|_| {
                    order.push_back(1);
                    Ok(())
                })
                .then(|_| {
                    order.push_back(2);
                    Err(Error::NotInitialized)
                })
                .then(|_| {
                    order.push_back(3);
                    Ok(())
                })
                .execute();

            assert_eq!(result, Err(Error::NotInitialized));
            assert_eq!(order, soroban_sdk::vec![env, 1, 2]);
        });
    }
}

    struct PassGuard;
    impl Guard for PassGuard {
        fn check(&self, _env: &Env, _ctx: &GuardContext) -> Result<(), Error> {
            Ok(())
        }
    }

    struct FailGuard;
    impl Guard for FailGuard {
        fn check(&self, _env: &Env, _ctx: &GuardContext) -> Result<(), Error> {
            Err(Error::Unauthorized)
        }
    }

    #[test]
    fn composition_preserves_first_error() {
        with_contract(|env| {
            let ctx = GuardContext::new(env);
            let result = GuardComposition::sequence(env)
                .then_guard(&FailGuard, &ctx)
                .then_guard(&PassGuard, &ctx)
                .execute();

            assert_eq!(result, Err(Error::Unauthorized));
        });
    }

    #[test]
    fn composition_runs_guards_in_order() {
        with_contract(|env| {
            let ctx = GuardContext::new(env);
            let result = GuardComposition::sequence(env)
                .then_guard(&PassGuard, &ctx)
                .then_guard(&PassGuard, &ctx)
                .execute();

            assert!(result.is_ok());
        });
    }

    #[test]
    fn builder_composes_arbitrary_guards() {
        with_contract(|env| {
            let ctx = GuardContext::new(env);
            let result = GuardBuilder::with(env)
                .guard(&PassGuard)
                .guard(&PassGuard)
                .run(&ctx);

            assert!(result.is_ok());
        });
    }

    #[test]
    fn builder_short_circuits_on_failure() {
        with_contract(|env| {
            let ctx = GuardContext::new(env);
            let result = GuardBuilder::with(env)
                .guard(&FailGuard)
                .guard(&PassGuard)
                .run(&ctx);

            assert_eq!(result, Err(Error::Unauthorized));
        });
    }
}
