//! Common interface or contract pattern for guard implementations.
//!
//! This module defines a standardized trait that all guard implementations
//! should follow, enabling consistent behavior and composition.
//!
//! # Acceptance Criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Define standard guard behavior | [`Guard`] trait |
//! | Ensure guards receive required contract context | [`GuardContext`] |
//! | Support reusable guard composition | Trait object design + examples |
//! | Add interface tests | Comprehensive test module |
//!
//! # Design
//!
//! The [`Guard`] trait is intentionally minimal and composable:
//! - **One responsibility**: evaluate a single pre-condition.
//! - **Standardized input**: [`GuardContext`] provides contract environment.
//! - **Standardized output**: [`crate::guard_result::GuardResult`] for consistency.
//! - **No side effects**: guards never mutate storage (read-only checks).
//!
//! This design enables:
//! - Easy testing and mocking.
//! - Clear composition via trait objects or combinators.
//! - Reuse across different entry points.
//!
//! # Usage
//!
//! ## Implementing a Guard
//!
//! ```rust,ignore
//! use soroban_sdk::{Address, Env};
//! use crate::guard_interface::{Guard, GuardContext};
//! use crate::guard_result::GuardResult;
//!
//! struct MyGuard;
//!
//! impl Guard for MyGuard {
//!     fn execute(&self, ctx: &GuardContext) -> GuardResult {
//!         // Perform read-only checks using ctx.env and ctx.caller
//!         GuardResult::Pass
//!     }
//!
//!     fn name(&self) -> &'static str {
//!         "my_guard"
//!     }
//! }
//! ```
//!
//! ## Composing Guards
//!
//! ```rust,ignore
//! let guards: Vec<Box<dyn Guard>> = vec![
//!     Box::new(AdminGuard),
//!     Box::new(PauseGuard),
//!     Box::new(TokenStateGuard),
//! ];
//!
//! for guard in guards {
//!     guard.execute(&ctx).to_contract_result()?;
//! }
//! ```

use soroban_sdk::{Address, Env};

use crate::guard_result::GuardResult;
use crate::types::TokenId;

// ─── Guard context ────────────────────────────────────────────────────────────

/// Contract context provided to guards during execution.
///
/// Encapsulates the minimal environment needed for a guard to evaluate
/// pre-conditions without knowing implementation details.
///
/// # Fields
///
/// - `env` — Soroban execution environment; used for storage reads, auth checks.
/// - `caller` — Address attempting the protected operation.
/// - `token_id` — Optional token identifier (for token-specific guards).
///
/// Guards that don't need all fields should ignore them.
#[derive(Clone)]
pub struct GuardContext {
    /// Soroban execution environment.
    pub env: Env,

    /// Address invoking the protected operation.
    pub caller: Address,

    /// Token identifier, if applicable.
    ///
    /// Populated for token-specific guards (e.g. transfer, freeze, royalty).
    /// Set to `None` for contract-level guards (e.g. pause, admin).
    pub token_id: Option<TokenId>,
}

impl GuardContext {
    /// Create a new guard context for a contract-level operation (no token_id).
    ///
    /// # Arguments
    ///
    /// * `env` — Soroban execution environment.
    /// * `caller` — Address invoking the operation.
    pub fn new(env: Env, caller: Address) -> Self {
        Self {
            env,
            caller,
            token_id: None,
        }
    }

    /// Create a new guard context for a token-specific operation.
    ///
    /// # Arguments
    ///
    /// * `env` — Soroban execution environment.
    /// * `caller` — Address invoking the operation.
    /// * `token_id` — Token identifier for this operation.
    pub fn with_token(env: Env, caller: Address, token_id: TokenId) -> Self {
        Self {
            env,
            caller,
            token_id: Some(token_id),
        }
    }

    /// Check whether this context is for a token-specific operation.
    pub fn is_token_operation(&self) -> bool {
        self.token_id.is_some()
    }

    /// Extract the token_id, panicking if not set.
    ///
    /// Use this in guards that require a token context.
    pub fn require_token_id(&self) -> TokenId {
        self.token_id
            .expect("guard context is missing required token_id")
    }
}

// ─── Guard trait ──────────────────────────────────────────────────────────────

/// Standardized guard interface for all contract authorization and validation
/// checks.
///
/// Implementations of [`Guard`] are responsible for evaluating a single
/// pre-condition in a read-only manner. Guards are composed by guard validators
/// or manually sequenced to protect contract operations.
///
/// # Invariants
///
/// - **No side effects** — guards never mutate contract storage.
/// - **Deterministic** — given the same context, a guard always returns the
///   same result.
/// - **Fail-fast** — guards should return immediately on the first failure
///   condition.
///
/// # Implementing
///
/// To implement a guard:
///
/// 1. Create a unit struct or zero-cost wrapper type.
/// 2. Implement [`execute`](Self::execute) to evaluate the pre-condition.
/// 3. Implement [`name`](Self::name) to return a debug-friendly identifier.
///
/// # Example
///
/// ```rust,ignore
/// struct PauseGuard;
///
/// impl Guard for PauseGuard {
///     fn execute(&self, ctx: &GuardContext) -> GuardResult {
///         if is_paused(&ctx.env) {
///             GuardResult::Fail(Error::ContractPaused)
///         } else {
///             GuardResult::Pass
///         }
///     }
///
///     fn name(&self) -> &'static str {
///         "pause_guard"
///     }
/// }
/// ```
pub trait Guard {
    /// Execute the guard and return the result.
    ///
    /// # Arguments
    ///
    /// * `ctx` — Guard context with environment, caller, and optional token_id.
    ///
    /// # Returns
    ///
    /// - [`GuardResult::Pass`] — pre-condition satisfied; operation may proceed.
    /// - [`GuardResult::Fail(err)`] — pre-condition failed; operation must be rejected
    ///   with the given error.
    fn execute(&self, ctx: &GuardContext) -> GuardResult;

    /// Return a debug-friendly name for this guard.
    ///
    /// Used for logging, error messages, and test diagnostics.
    /// Should be lowercase with underscores (e.g. `"admin_guard"`).
    fn name(&self) -> &'static str;
}

// ─── Blanket implementations ──────────────────────────────────────────────────

impl Guard for Result<(), crate::types::Error> {
    fn execute(&self, _ctx: &GuardContext) -> GuardResult {
        match self {
            Ok(()) => GuardResult::Pass,
            Err(err) => GuardResult::Fail(*err),
        }
    }

    fn name(&self) -> &'static str {
        "result_guard"
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Error;
    use soroban_sdk::testutils::Address as AddressTestUtils;

    // Mock guard for testing
    struct MockPassGuard;

    impl Guard for MockPassGuard {
        fn execute(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::Pass
        }

        fn name(&self) -> &'static str {
            "mock_pass_guard"
        }
    }

    struct MockFailGuard(Error);

    impl Guard for MockFailGuard {
        fn execute(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::Fail(self.0)
        }

        fn name(&self) -> &'static str {
            "mock_fail_guard"
        }
    }

    #[test]
    fn test_guard_context_new() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);

        let ctx = GuardContext::new(env.clone(), caller.clone());

        assert_eq!(ctx.caller, caller);
        assert_eq!(ctx.token_id, None);
        assert!(!ctx.is_token_operation());
    }

    #[test]
    fn test_guard_context_with_token() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let token_id = 42u64;

        let ctx = GuardContext::with_token(env.clone(), caller.clone(), token_id);

        assert_eq!(ctx.caller, caller);
        assert_eq!(ctx.token_id, Some(token_id));
        assert!(ctx.is_token_operation());
        assert_eq!(ctx.require_token_id(), token_id);
    }

    #[test]
    fn test_mock_pass_guard() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let ctx = GuardContext::new(env, caller);

        let guard = MockPassGuard;
        let result = guard.execute(&ctx);

        assert_eq!(result, GuardResult::Pass);
        assert_eq!(guard.name(), "mock_pass_guard");
    }

    #[test]
    fn test_mock_fail_guard() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let ctx = GuardContext::new(env, caller);

        let guard = MockFailGuard(Error::Unauthorized);
        let result = guard.execute(&ctx);

        assert_eq!(result, GuardResult::Fail(Error::Unauthorized));
        assert_eq!(guard.name(), "mock_fail_guard");
    }

    #[test]
    fn test_guard_trait_object_pass() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let ctx = GuardContext::new(env, caller);

        let guard: Box<dyn Guard> = Box::new(MockPassGuard);
        let result = guard.execute(&ctx);

        assert_eq!(result, GuardResult::Pass);
    }

    #[test]
    fn test_guard_trait_object_fail() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let ctx = GuardContext::new(env, caller);

        let guard: Box<dyn Guard> = Box::new(MockFailGuard(Error::ContractPaused));
        let result = guard.execute(&ctx);

        assert_eq!(result, GuardResult::Fail(Error::ContractPaused));
    }

    #[test]
    #[should_panic(expected = "guard context is missing required token_id")]
    fn test_require_token_id_panics_when_missing() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let ctx = GuardContext::new(env, caller);

        let _ = ctx.require_token_id();
    }
}
