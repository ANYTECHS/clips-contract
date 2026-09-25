//! Validator interface (issue #1084).
//!
//! Defines the common interface (pattern) implemented by every contract
//! validator so that validation logic is reusable across entry points and
//! always returns a consistent [`crate::validators::ValidationResult`].
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Define standard validation behavior | [`Validator`] trait |
//! | Support contract context and input data | [`ValidationContext`] + generic `Input` type |
//! | Return consistent validation results | Every `validate` returns [`crate::validators::ValidationResult`] |
//! | Add interface tests | Tests module with comprehensive coverage |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::validators::{ValidationContext, Validator};
//!
//! pub struct RoyaltyBpsValidator;
//!
//! impl Validator<u32> for RoyaltyBpsValidator {
//!     fn validate(&self, ctx: &ValidationContext<'_>, bps: &u32) -> ValidationResult {
//!         let _ = ctx; // contract context available when needed
//!         ValidationResult::from_result(crate::royalty_validator::validate_royalty_bps(*bps))
//!     }
//! }
//!
//! let ctx = ValidationContext::new(&env);
//! let result = RoyaltyBpsValidator.validate(&ctx, &500);
//! result.require()?;
//! ```
//!
//! Function-style validators can be lifted into the interface with
//! [`FnValidator`] without writing a new type:
//!
//! ```rust,ignore
//! let validator = FnValidator::new(|ctx: &ValidationContext<'_>, bps: &u32| {
//!     let _ = ctx;
//!     ValidationResult::from_result(crate::royalty_validator::validate_royalty_bps(*bps))
//! });
//! ```

use soroban_sdk::{Address, Env};

use crate::types::TokenId;
use crate::validators::result::ValidationResult;

// ─── Contract context ─────────────────────────────────────────────────────────

/// Shared contract context passed to every validator.
///
/// Carries the live Soroban [`Env`] plus the optional identities and resource
/// references most validators need, so individual `Input` types stay focused
/// on their own domain data:
///
/// - `env` — contract environment (storage, ledger, events).
/// - `caller` — address invoking the operation, when known.
/// - `token_id` — token under validation, when applicable.
///
/// Use the builder-style `with_*` helpers to attach optional data; the
/// timestamp is read from the ledger at construction time.
pub struct ValidationContext<'a> {
    /// Live contract environment.
    pub env: &'a Env,
    /// Address invoking the operation, when known.
    pub caller: Option<&'a Address>,
    /// Token under validation, when applicable.
    pub token_id: Option<TokenId>,
    /// Ledger timestamp captured at construction.
    pub timestamp: u64,
}

impl<'a> ValidationContext<'a> {
    /// Build a context carrying only the contract environment.
    pub fn new(env: &'a Env) -> Self {
        let timestamp = env.ledger().timestamp();
        ValidationContext {
            env,
            caller: None,
            token_id: None,
            timestamp,
        }
    }

    /// Attach the invoking caller to the context.
    pub fn with_caller(mut self, caller: &'a Address) -> Self {
        self.caller = Some(caller);
        self
    }

    /// Attach the token under validation to the context.
    pub fn with_token(mut self, token_id: TokenId) -> Self {
        self.token_id = Some(token_id);
        self
    }
}

// ─── Standard validation behavior ─────────────────────────────────────────────

/// Common interface implemented by every contract validator.
///
/// The generic `Input` type is the domain data under
/// validation (an address, a token id, a royalty struct, a mint request,
/// …). The [`ValidationContext`] supplies the shared contract context
/// (environment, caller, token, timestamp) so validators never need
/// ad-hoc parameter lists.
///
/// Every implementation returns the standardized
/// [`ValidationResult`], giving pipelines, guards, and entry points one
/// consistent shape to match on regardless of domain.
pub trait Validator<Input> {
    /// Validate `input` against the contract `context`.
    ///
    /// Returns [`ValidationResult::Valid`] when every constraint holds, or
    /// [`ValidationResult::Invalid`] carrying the centralized contract error
    /// describing the first failure.
    fn validate(&self, ctx: &ValidationContext<'_>, input: &Input) -> ValidationResult;
}

// ─── Function adapter ─────────────────────────────────────────────────────────

/// Adapter that turns any matching function or closure into a [`Validator`].
///
/// Useful for lifting existing function-style validators (or small inline
/// checks in tests) into the standard interface without declaring a new type.
pub struct FnValidator<F> {
    inner: F,
}

impl<F> FnValidator<F> {
    /// Wrap `inner` as a validator.
    pub fn new(inner: F) -> Self {
        FnValidator { inner }
    }
}

impl<F, Input> Validator<Input> for FnValidator<F>
where
    F: Fn(&ValidationContext<'_>, &Input) -> ValidationResult,
{
    fn validate(&self, ctx: &ValidationContext<'_>, input: &Input) -> ValidationResult {
        (self.inner)(ctx, input)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Error;
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

    /// Minimal validator used to prove the interface contract: accepts even
    /// basis points, rejects odd ones.
    struct EvenBpsValidator;

    impl Validator<u32> for EvenBpsValidator {
        fn validate(&self, _ctx: &ValidationContext<'_>, input: &u32) -> ValidationResult {
            ValidationResult::from_condition(
                input % 2 == 0,
                Error::InvalidBasisPoints,
            )
        }
    }

    /// Validator that reads contract context (caller) in addition to input.
    struct CallerMatchesValidator;

    impl Validator<Address> for CallerMatchesValidator {
        fn validate(
            &self,
            ctx: &ValidationContext<'_>,
            input: &Address,
        ) -> ValidationResult {
            match ctx.caller {
                Some(caller) if caller == input => ValidationResult::Valid,
                _ => ValidationResult::Invalid(Error::Unauthorized),
            }
        }
    }

    /// Validator that reads the token id from the context.
    struct TokenMatchesValidator;

    impl Validator<TokenId> for TokenMatchesValidator {
        fn validate(
            &self,
            ctx: &ValidationContext<'_>,
            input: &TokenId,
        ) -> ValidationResult {
            match ctx.token_id {
                Some(token_id) if token_id == *input => ValidationResult::Valid,
                Some(_) => ValidationResult::Invalid(Error::TokenNotFound),
                None => ValidationResult::Invalid(Error::InvalidConfig),
            }
        }
    }

    // ── Standard validation behavior ──────────────────────────────────────

    #[test]
    fn validator_returns_valid_for_acceptable_input() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            assert!(EvenBpsValidator.validate(&ctx, &500).is_valid());
        });
    }

    #[test]
    fn validator_returns_consistent_invalid_result() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            assert_eq!(
                EvenBpsValidator.validate(&ctx, &501),
                ValidationResult::Invalid(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn validator_result_converts_into_centralized_error() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let outcome: Result<(), Error> =
                EvenBpsValidator.validate(&ctx, &501).into();
            assert_eq!(outcome, Err(Error::InvalidBasisPoints));
        });
    }

    // ── Contract context and input data ───────────────────────────────────

    #[test]
    fn context_carries_env_caller_token_and_timestamp() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx = ValidationContext::new(env)
                .with_caller(&caller)
                .with_token(7);

            assert_eq!(ctx.caller, Some(&caller));
            assert_eq!(ctx.token_id, Some(7));
            assert_eq!(ctx.timestamp, env.ledger().timestamp());
        });
    }

    #[test]
    fn validator_can_read_caller_from_context() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let other = Address::generate(env);

            let ctx = ValidationContext::new(env).with_caller(&caller);
            assert!(CallerMatchesValidator.validate(&ctx, &caller).is_valid());
            assert_eq!(
                CallerMatchesValidator.validate(&ctx, &other),
                ValidationResult::Invalid(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn validator_rejects_when_context_caller_missing() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx = ValidationContext::new(env);
            assert_eq!(
                CallerMatchesValidator.validate(&ctx, &caller),
                ValidationResult::Invalid(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn validator_can_read_token_id_from_context() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env).with_token(42);
            assert!(TokenMatchesValidator.validate(&ctx, &42).is_valid());
            assert_eq!(
                TokenMatchesValidator.validate(&ctx, &43),
                ValidationResult::Invalid(Error::TokenNotFound)
            );

            let bare = ValidationContext::new(env);
            assert_eq!(
                TokenMatchesValidator.validate(&bare, &42),
                ValidationResult::Invalid(Error::InvalidConfig)
            );
        });
    }

    // ── Function adapter ──────────────────────────────────────────────────

    #[test]
    fn fn_validator_lifts_closures_into_interface() {
        with_contract(|env| {
            let validator = FnValidator::new(
                |_ctx: &ValidationContext<'_>, bps: &u32| {
                    ValidationResult::from_result(
                        crate::royalty_validator::validate_royalty_bps(*bps),
                    )
                },
            );
            let ctx = ValidationContext::new(env);
            assert!(validator.validate(&ctx, &500).is_valid());
            assert_eq!(
                validator.validate(&ctx, &20_000),
                ValidationResult::Invalid(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn interface_wraps_existing_result_style_validators() {
        with_contract(|env| {
            // Any `Result<(), Error>` validator converts losslessly, proving
            // the interface composes with the existing codebase.
            let validator = FnValidator::new(
                |_ctx: &ValidationContext<'_>, bps: &u32| {
                    crate::royalty_validator::validate_royalty_bps(*bps).into()
                },
            );
            let ctx = ValidationContext::new(env);
            assert!(validator.validate(&ctx, &10_000).is_valid());
            assert!(validator.validate(&ctx, &10_001).is_invalid());
        });
    }
}
