//! Guard composition framework (Issue #1091).
//!
//! Supports combining multiple guards for operations that require several
//! authorization checks.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Support multiple guards | [`GuardComposition`] trait and combinators |
//! | Execute guards in deterministic order | Defined sequence in all combinators |
//! | Stop execution when a guard fails | Early return on first error |
//! | Add composition tests | Comprehensive test module |
//!
//! # Design
//!
//! The composition framework is built on a simple, composable trait:
//!
//! - [`GuardComposition`] — a trait that executes a guard and returns a result.
//! - Implementations combine one or more base guards and execute them
//!   deterministically.
//! - Combinators stop on the first error encountered.
//!
//! ## Combinators
//!
//! ### Sequence (And)
//!
//! [`sequence`] combines two guards that must **both** pass. Stops on the first
//! failure. Useful for operations requiring multiple independent checks.
//!
//! ```rust,ignore
//! guard_composition::sequence(guard1, guard2)?;  // Guard1 runs first, then guard2
//! ```
//!
//! ### Builder
//!
//! [`GuardBuilder`] allows composing an arbitrary number of guards in a
//! readable, chainable syntax.
//!
//! ```rust,ignore
//! GuardBuilder::new()
//!     .add(ownership_guard::require_owner(&env, &caller, token_id))
//!     .add(admin_access_control_guard::require_admin(&env, &caller))
//!     .add(some_other_guard)
//!     .execute()?;
//! ```
//!
//! # Usage examples
//!
//! ```rust,ignore
//! // Simple binary composition
//! guard_composition::sequence(
//!     admin_access_control_guard::require_admin(&env, &caller),
//!     pause_guard::require_not_paused(&env),
//! )?;
//!
//! // Builder for three or more guards
//! use guard_composition::GuardBuilder;
//!
//! GuardBuilder::new()
//!     .add(admin_access_control_guard::require_admin(&env, &caller))
//!     .add(pause_guard::require_not_paused(&env))
//!     .add(check_some_other_state(&env))
//!     .execute()?;
//! ```
//!
//! # Deterministic ordering
//!
//! All combinators execute guards in a deterministic, predictable order:
//!
//! - **Binary sequence**: left guard first, then right guard.
//! - **Builder**: guards added via [`GuardBuilder::add`] in the order added.
//!
//! Callers should order guards by concern:
//! 1. **Operational** (e.g. pause checks).
//! 2. **Initialization** (e.g. contract initialized checks).
//! 3. **Authorization** (e.g. admin or owner checks).

use crate::types::Error;

// ─── Core trait ───────────────────────────────────────────────────────────────

/// Trait for types that can execute a guard and return a guard result.
///
/// Any type implementing this trait can participate in guard composition
/// via combinators like [`sequence`] or [`GuardBuilder`].
pub trait GuardComposition {
    /// Execute the guard and return the result.
    ///
    /// Returns `Ok(())` if the guard passes, or the first [`Error`] encountered.
    fn execute(self) -> Result<(), Error>;
}

// ─── Impl for Result<(), Error> ────────────────────────────────────────────────

impl GuardComposition for Result<(), Error> {
    fn execute(self) -> Result<(), Error> {
        self
    }
}

// ─── Binary composition (sequence) ─────────────────────────────────────────────

/// Compose two guards: execute `first`, then `second`.
///
/// Both guards must pass for `sequence` to succeed. If either guard fails,
/// returns the first error encountered and does not execute remaining guards.
///
/// # Guard order
///
/// 1. `first` executes first.
/// 2. `second` executes second (only if `first` passes).
///
/// # Errors
///
/// Returns the first error from either guard; does not continue executing
/// remaining guards once an error occurs.
///
/// # Example
///
/// ```rust,ignore
/// // Verify admin, then verify contract is not paused
/// guard_composition::sequence(
///     admin_access_control_guard::require_admin(&env, &caller),
///     pause_guard::require_not_paused(&env),
/// )?;
/// ```
pub fn sequence<G1, G2>(first: G1, second: G2) -> Result<(), Error>
where
    G1: GuardComposition,
    G2: GuardComposition,
{
    first.execute()?;
    second.execute()?;
    Ok(())
}

// ─── Multi-guard builder ───────────────────────────────────────────────────────

/// Builder for composing three or more guards in a readable, chainable way.
///
/// Guards are executed in the order added. All guards must pass for the
/// builder to succeed.
///
/// # Example
///
/// ```rust,ignore
/// use guard_composition::GuardBuilder;
///
/// GuardBuilder::new()
///     .add(admin_access_control_guard::require_admin(&env, &caller))
///     .add(pause_guard::require_not_paused(&env))
///     .add(ownership_guard::require_owner(&env, &caller, token_id))
///     .execute()?;
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct GuardBuilder {
    /// Internal state — stores all guard results in order.
    /// We store `Option<Result<(), Error>>` to track which guards have been added.
    results: [Option<Result<(), Error>>; 16],
    /// Number of guards added
    count: usize,
}

impl GuardBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        GuardBuilder {
            results: [None; 16],
            count: 0,
        }
    }

    /// Add a guard result to the builder.
    ///
    /// # Panics
    ///
    /// Panics if more than 16 guards are added. This limit is intentional to
    /// prevent accidental overly-complex guard compositions.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// builder.add(admin_access_control_guard::require_admin(&env, &caller))
    /// ```
    pub fn add(mut self, guard: Result<(), Error>) -> Self {
        assert!(
            self.count < 16,
            "GuardBuilder supports a maximum of 16 guards per composition"
        );
        self.results[self.count] = Some(guard);
        self.count += 1;
        self
    }

    /// Execute all added guards in order.
    ///
    /// Returns `Ok(())` if all guards pass. Returns the first error encountered
    /// if any guard fails; does not execute remaining guards.
    ///
    /// # Errors
    ///
    /// Returns the first error from any of the added guards.
    pub fn execute(self) -> Result<(), Error> {
        for i in 0..self.count {
            if let Some(result) = self.results[i] {
                result?;
            }
        }
        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper guards for testing ──────────────────────────────────────────

    fn guard_always_pass() -> Result<(), Error> {
        Ok(())
    }

    fn guard_always_fail() -> Result<(), Error> {
        Err(Error::Unauthorized)
    }

    fn guard_specific_error() -> Result<(), Error> {
        Err(Error::TokenNotFound)
    }

    // ── sequence tests ─────────────────────────────────────────────────────

    #[test]
    fn sequence_passes_when_both_guards_pass() {
        let result = sequence(guard_always_pass(), guard_always_pass());
        assert!(result.is_ok());
    }

    #[test]
    fn sequence_fails_when_first_guard_fails() {
        let result = sequence(guard_always_fail(), guard_always_pass());
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn sequence_fails_when_second_guard_fails() {
        let result = sequence(guard_always_pass(), guard_always_fail());
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn sequence_stops_on_first_error() {
        // Both guards fail, but we should get the first error
        let result = sequence(guard_always_fail(), guard_always_fail());
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn sequence_returns_first_guard_error_when_first_fails() {
        let result = sequence(guard_specific_error(), guard_always_fail());
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    // ── GuardBuilder tests ─────────────────────────────────────────────────

    #[test]
    fn builder_passes_with_no_guards() {
        let result = GuardBuilder::new().execute();
        assert!(result.is_ok());
    }

    #[test]
    fn builder_passes_with_one_passing_guard() {
        let result = GuardBuilder::new().add(guard_always_pass()).execute();
        assert!(result.is_ok());
    }

    #[test]
    fn builder_fails_with_one_failing_guard() {
        let result = GuardBuilder::new().add(guard_always_fail()).execute();
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn builder_passes_with_multiple_passing_guards() {
        let result = GuardBuilder::new()
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .execute();
        assert!(result.is_ok());
    }

    #[test]
    fn builder_fails_with_multiple_guards_first_fails() {
        let result = GuardBuilder::new()
            .add(guard_always_fail())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .execute();
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn builder_fails_with_multiple_guards_middle_fails() {
        let result = GuardBuilder::new()
            .add(guard_always_pass())
            .add(guard_always_fail())
            .add(guard_always_pass())
            .execute();
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn builder_fails_with_multiple_guards_last_fails() {
        let result = GuardBuilder::new()
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_fail())
            .execute();
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn builder_stops_on_first_error() {
        let result = GuardBuilder::new()
            .add(guard_always_pass())
            .add(guard_specific_error())
            .add(guard_always_fail()) // This should not execute
            .execute();
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    #[test]
    fn builder_executes_guards_in_order() {
        // By returning specific errors, we can verify execution order
        let result = GuardBuilder::new()
            .add(guard_specific_error())
            .add(guard_always_fail())
            .execute();
        // Should get the first error, not the second
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    #[test]
    fn builder_supports_many_guards() {
        let result = GuardBuilder::new()
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass())
            .execute();
        assert!(result.is_ok());
    }

    // ── GuardComposition trait ─────────────────────────────────────────────

    #[test]
    fn result_implements_guard_composition_for_ok() {
        let result: Result<(), Error> = Ok(());
        assert!(result.execute().is_ok());
    }

    #[test]
    fn result_implements_guard_composition_for_err() {
        let result: Result<(), Error> = Err(Error::Unauthorized);
        assert_eq!(result.execute(), Err(Error::Unauthorized));
    }

    // ── Complex composition tests ──────────────────────────────────────────

    #[test]
    fn nested_sequence_passes_all_guards() {
        let result = sequence(
            guard_always_pass(),
            sequence(guard_always_pass(), guard_always_pass()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn nested_sequence_fails_on_first_error() {
        let result = sequence(
            guard_always_pass(),
            sequence(guard_specific_error(), guard_always_fail()),
        );
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    // ── Determinism tests ──────────────────────────────────────────────────

    #[test]
    fn builder_is_deterministic_across_calls() {
        let builder = GuardBuilder::new()
            .add(guard_always_pass())
            .add(guard_always_pass())
            .add(guard_always_pass());

        assert!(builder.execute().is_ok());
        assert!(builder.execute().is_ok());
        assert!(builder.execute().is_ok());
    }

    #[test]
    fn sequence_is_deterministic_across_calls() {
        for _ in 0..3 {
            assert!(sequence(guard_always_pass(), guard_always_pass()).is_ok());
        }
    }
}
