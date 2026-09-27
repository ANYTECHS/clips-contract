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
//! | Define guard success state | [`GuardSuccess`] |
//! | Define guard failure state | [`GuardFailure`] |
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
//! result.to_contract_error()
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
    }
}
