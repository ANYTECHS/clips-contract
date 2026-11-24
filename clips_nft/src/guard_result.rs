//! Guard result types — standardized representation of guard success and failure
//! states.
//!
//! This module defines the contract for representing guard execution outcomes
//! in a consistent, reusable way.
//!
//! # Acceptance Criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Define guard success state | [`GuardResult::Pass`] |
//! | Define guard failure state | [`GuardResult::Fail`] |
//! | Support contract error integration | Auto-converts to [`crate::types::Error`] |
//! | Add unit tests | Comprehensive test module |
//!
//! # Design
//!
//! All guards execute in two phases:
//!
//! 1. **Check** — evaluate pre-conditions without side effects.
//! 2. **Convert to Result** — map check result to `Result<(), Error>` for use
//!    in guard composition and contract entry points.
//!
//! [`GuardResult`] abstracts both phases:
//! - Guards return a [`GuardResult`].
//! - Callers convert it to `Result<(), Error>` when needed.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Guard evaluates pre-conditions
//! let result = some_guard(env, caller)?;
//!
//! // Convert to contract result when needed
//! result.to_contract_result()
//! ```

use crate::types::Error;

// ─── Primary result type ──────────────────────────────────────────────────────

/// Outcome of a guard evaluation.
///
/// Represents the complete state following a guard check, including any
/// diagnostics or metadata useful for error handling and logging.
///
/// # Variants
///
/// - [`GuardResult::Pass`] — guard check succeeded; operation may proceed.
/// - [`GuardResult::Fail`] — guard check failed; operation must be rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardResult {
    /// Guard check succeeded; operation may proceed.
    Pass,

    /// Guard check failed.
    ///
    /// Associates a specific contract error with the failure, making error
    /// flow explicit and reducing silent failures.
    Fail(Error),
}

// ─── Constructors ─────────────────────────────────────────────────────────────

impl GuardResult {
    /// Construct a successful guard result.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = GuardResult::pass();
    /// assert!(result.passed());
    /// ```
    pub fn pass() -> Self {
        GuardResult::Pass
    }

    /// Construct a failed guard result carrying the given contract error.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = GuardResult::fail(Error::Unauthorized);
    /// assert!(result.failed());
    /// ```
    pub fn fail(err: Error) -> Self {
        GuardResult::Fail(err)
    }
}

// ─── Conversions ──────────────────────────────────────────────────────────────

impl GuardResult {
    /// Convert the guard result to a Soroban contract result.
    ///
    /// - `Pass` → `Ok(())`
    /// - `Fail(err)` → `Err(err)`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = guard_check(env, caller);
    /// result.to_contract_result()?;  // Fail-fast on error
    /// ```
    pub fn to_contract_result(self) -> Result<(), Error> {
        match self {
            GuardResult::Pass => Ok(()),
            GuardResult::Fail(err) => Err(err),
        }
    }

    /// Return `true` if the guard passed, `false` otherwise.
    pub fn passed(&self) -> bool {
        matches!(self, GuardResult::Pass)
    }

    /// Return `true` if the guard failed, `false` otherwise.
    pub fn failed(&self) -> bool {
        matches!(self, GuardResult::Fail(_))
    }

    /// Extract the error if the guard failed, or `None` if it passed.
    pub fn error(&self) -> Option<Error> {
        match self {
            GuardResult::Pass => None,
            GuardResult::Fail(err) => Some(*err),
        }
    }

    /// Combine two guard results, returning the first failure encountered.
    ///
    /// This supports reusable guard composition: multiple guards can be
    /// evaluated and their outcomes folded together, short-circuiting on the
    /// first failure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let combined = guard_a(env, caller)
    ///     .and_then(guard_b(env, caller))
    ///     .and_then(guard_c(env, caller));
    /// combined.to_contract_result()?;
    /// ```
    pub fn and_then(self, next: GuardResult) -> GuardResult {
        match self {
            GuardResult::Pass => next,
            fail => fail,
        }
    }
}

impl From<GuardResult> for Result<(), Error> {
    fn from(result: GuardResult) -> Self {
        result.to_contract_result()
    }
}

impl From<Result<(), Error>> for GuardResult {
    fn from(result: Result<(), Error>) -> Self {
        match result {
            Ok(()) => GuardResult::Pass,
            Err(err) => GuardResult::Fail(err),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_result_pass() {
        let result = GuardResult::Pass;
        assert!(result.passed());
        assert!(!result.failed());
        assert_eq!(result.error(), None);
    }

    #[test]
    fn test_guard_result_fail() {
        let result = GuardResult::Fail(Error::Unauthorized);
        assert!(!result.passed());
        assert!(result.failed());
        assert_eq!(result.error(), Some(Error::Unauthorized));
    }

    #[test]
    fn test_guard_result_pass_constructor() {
        let result = GuardResult::pass();
        assert_eq!(result, GuardResult::Pass);
        assert!(result.passed());
        assert_eq!(result.error(), None);
    }

    #[test]
    fn test_guard_result_fail_constructor() {
        let result = GuardResult::fail(Error::Unauthorized);
        assert_eq!(result, GuardResult::Fail(Error::Unauthorized));
        assert!(result.failed());
        assert_eq!(result.error(), Some(Error::Unauthorized));
    }

    #[test]
    fn test_guard_result_to_contract_result_pass() {
        let result = GuardResult::Pass;
        assert_eq!(result.to_contract_result(), Ok(()));
    }

    #[test]
    fn test_guard_result_to_contract_result_fail() {
        let result = GuardResult::Fail(Error::TokenNotFound);
        assert_eq!(result.to_contract_result(), Err(Error::TokenNotFound));
    }

    #[test]
    fn test_from_guard_result_to_result() {
        let result: Result<(), Error> = GuardResult::Pass.into();
        assert_eq!(result, Ok(()));

        let result: Result<(), Error> = GuardResult::Fail(Error::Unauthorized).into();
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn test_from_result_to_guard_result() {
        let result: GuardResult = Ok::<(), Error>(()).into();
        assert_eq!(result, GuardResult::Pass);

        let result: GuardResult = Err::<(), Error>(Error::ContractPaused).into();
        assert_eq!(result, GuardResult::Fail(Error::ContractPaused));
    }

    #[test]
    fn test_guard_result_roundtrip() {
        let original = GuardResult::Fail(Error::Unauthorized);
        let as_result: Result<(), Error> = original.into();
        let roundtrip: GuardResult = as_result.into();
        assert_eq!(roundtrip, original);

        let original = GuardResult::Pass;
        let as_result: Result<(), Error> = original.into();
        let roundtrip: GuardResult = as_result.into();
        assert_eq!(roundtrip, original);
    }

    #[test]
    fn test_guard_result_equality() {
        assert_eq!(GuardResult::Pass, GuardResult::Pass);
        assert_eq!(
            GuardResult::Fail(Error::Unauthorized),
            GuardResult::Fail(Error::Unauthorized)
        );
        assert_ne!(
            GuardResult::Fail(Error::Unauthorized),
            GuardResult::Fail(Error::TokenNotFound)
        );
        assert_ne!(GuardResult::Pass, GuardResult::Fail(Error::Unauthorized));
    }

    #[test]
    fn test_pass_fail_roundtrip_conversion() {
        // Pass -> Ok(()) -> Pass
        let pass = GuardResult::Pass;
        let as_result = pass.to_contract_result();
        assert_eq!(as_result, Ok(()));
        assert_eq!(GuardResult::from(as_result), GuardResult::Pass);

        // Fail -> Err(err) -> Fail
        let fail = GuardResult::Fail(Error::Unauthorized);
        let as_result = fail.to_contract_result();
        assert_eq!(as_result, Err(Error::Unauthorized));
        assert_eq!(GuardResult::from(as_result), fail);
    }

    #[test]
    fn test_and_then_composition_pass_pass() {
        let combined = GuardResult::Pass.and_then(GuardResult::Pass);
        assert_eq!(combined, GuardResult::Pass);
    }

    #[test]
    fn test_and_then_composition_fail_short_circuits() {
        let combined = GuardResult::Fail(Error::Unauthorized).and_then(GuardResult::Pass);
        assert_eq!(combined, GuardResult::Fail(Error::Unauthorized));

        let combined = GuardResult::Pass.and_then(GuardResult::Fail(Error::TokenNotFound));
        assert_eq!(combined, GuardResult::Fail(Error::TokenNotFound));

        let combined =
            GuardResult::Fail(Error::Unauthorized).and_then(GuardResult::Fail(Error::TokenNotFound));
        assert_eq!(combined, GuardResult::Fail(Error::Unauthorized));
    }
}
}
