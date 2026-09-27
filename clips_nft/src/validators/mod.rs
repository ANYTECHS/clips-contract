//! Centralized validator module (issue #1083).
//!
//! A dedicated module organizing all smart contract validation logic behind
//! one reusable interface. Previously, validation lived in scattered
//! function-style modules (`mint_validator`, `royalty_validator`,
//! `config_validator`, …) with ad-hoc signatures; this module is the single
//! entry point for the standardized pattern:
//!
//! - [`result::ValidationResult`] (issue #1085) — the standardized
//!   success/failure structure wrapping the centralized [`crate::types::Error`].
//! - [`interface::Validator`] (issue #1084) — the common interface taking a
//!   [`interface::ValidationContext`] (contract context) plus typed input data.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Create a centralized validator structure | This module + re-exports |
//! | Separate validators by domain | Domain table + namespaced helpers below |
//! | Make validators reusable across contract functions | [`Validator`] + [`run_validator`] + [`validate_all`] |
//! | Add basic module tests | Tests module with comprehensive coverage |
//!
//! # Domain organization
//!
//! Domain validators remain in their focused modules; this module maps each
//! domain to its home so new code starts here instead of guessing:
//!
//! | Domain | Existing module | Standardized usage |
//! |--------|-----------------|--------------------|
//! | Royalty config | [`crate::royalty_validator`] | Wrap with [`FnValidator`] or implement [`Validator`] |
//! | Royalty asset | [`crate::royalty_asset_validator`] | Wrap with [`FnValidator`] |
//! | Royalty recipient | [`crate::royalty_recipient_validator`] | Wrap with [`FnValidator`] |
//! | Mint requests | [`crate::mint_validator`] | Wrap with [`FnValidator`] |
//! | Purchase | [`crate::purchase_validator`] | Wrap with [`FnValidator`] |
//! | Config | [`crate::config_validator`] | Wrap with [`FnValidator`] |
//! | Storage | [`crate::storage_validator`] | Wrap with [`FnValidator`] |
//! | Deductions | [`crate::transaction_deduction_validator`] | Wrap with [`FnValidator`] |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::validators::{run_validator, validate_all, FnValidator, ValidationContext};
//!
//! // Reuse one validator from several entry points.
//! let bps_check = FnValidator::new(|ctx: &ValidationContext<'_>, bps: &u32| {
//!     let _ = ctx;
//!     crate::royalty_validator::validate_royalty_bps(*bps).into()
//! });
//! let ctx = ValidationContext::new(&env);
//! run_validator(&bps_check, &ctx, &500)?;
//!
//! // Run several independent checks; the first failure wins.
//! validate_all(&[
//!     crate::royalty_validator::validate_royalty_bps(500).into(),
//!     crate::royalty_validator::validate_royalty_bps(700).into(),
//! ])?;
//! ```

pub mod interface;
pub mod result;

pub use interface::{FnValidator, ValidationContext, Validator};
pub use result::ValidationResult;

use crate::types::Error;

// ─── Reusable entry points ────────────────────────────────────────────────────

/// Run a single standardized validator against contract context and input.
///
/// This is the primary reusable integration point: any [`Validator`] can be
/// invoked from any contract function with one call, and the standardized
/// [`ValidationResult`] is converted into `Result<(), Error>` for `?`
/// propagation.
pub fn run_validator<V, Input>(
    validator: &V,
    ctx: &ValidationContext<'_>,
    input: &Input,
) -> Result<(), Error>
where
    V: Validator<Input>,
{
    validator.validate(ctx, input).into_result()
}

/// Run pre-computed validation results in order, stopping at the first failure.
///
/// Useful when callers already hold [`ValidationResult`] values (for example,
/// converted from existing `Result<(), Error>` validators) and want pipeline
/// semantics without re-running validators:
///
/// ```rust,ignore
/// validators::validate_all(&[result_a, result_b, result_c])?;
/// ```
///
/// Returns `Ok(())` when every result is [`ValidationResult::Valid`]; otherwise
/// returns the first enclosed [`Error`].
pub fn validate_all(results: &[ValidationResult]) -> Result<(), Error> {
    for result in results {
        result.require()?;
    }
    Ok(())
}

/// Convert any conventional `Result<(), Error>` validator outcome into the
/// standardized [`ValidationResult`].
///
/// Convenience alias for [`ValidationResult::from_result`] so centralized
/// call sites read uniformly: `validators::standardize(existing_check(...))`.
pub const fn standardize(result: Result<(), Error>) -> ValidationResult {
    ValidationResult::from_result(result)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    struct AcceptAll;
    struct RejectAll;

    impl Validator<u32> for AcceptAll {
        fn validate(&self, _ctx: &ValidationContext<'_>, _input: &u32) -> ValidationResult {
            ValidationResult::Valid
        }
    }

    impl Validator<u32> for RejectAll {
        fn validate(&self, _ctx: &ValidationContext<'_>, _input: &u32) -> ValidationResult {
            ValidationResult::Invalid(Error::InvalidConfig)
        }
    }

    // ── Centralized structure ─────────────────────────────────────────────

    #[test]
    fn module_reexports_result_and_interface() {
        with_contract(|env| {
            // The centralized module exposes both halves of the pattern.
            let ctx = ValidationContext::new(env);
            let ok: ValidationResult = ValidationResult::Valid;
            assert!(ok.is_valid());

            let adapted = FnValidator::new(
                |_ctx: &ValidationContext<'_>, input: &u32| {
                    ValidationResult::from_condition(*input > 0, Error::InvalidConfig)
                },
            );
            assert!(run_validator(&adapted, &ctx, &1).is_ok());
        });
    }

    // ── Reusability across contract functions ─────────────────────────────

    #[test]
    fn same_validator_is_reusable_across_calls() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let validator = AcceptAll;
            // One validator instance serves every call site.
            assert!(run_validator(&validator, &ctx, &1).is_ok());
            assert!(run_validator(&validator, &ctx, &2).is_ok());
            assert!(run_validator(&validator, &ctx, &3).is_ok());
        });
    }

    #[test]
    fn run_validator_propagates_failure_as_centralized_error() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            assert_eq!(
                run_validator(&RejectAll, &ctx, &1),
                Err(Error::InvalidConfig)
            );
        });
    }

    #[test]
    fn run_validator_composes_with_existing_domain_validators() {
        with_contract(|env| {
            let ctx = ValidationContext::new(env);
            let royalty_bps = FnValidator::new(
                |_ctx: &ValidationContext<'_>, bps: &u32| {
                    crate::royalty_validator::validate_royalty_bps(*bps).into()
                },
            );
            assert!(run_validator(&royalty_bps, &ctx, &500).is_ok());
            assert_eq!(
                run_validator(&royalty_bps, &ctx, &20_000),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    // ── Domain separation ─────────────────────────────────────────────────

    #[test]
    fn domain_validators_share_one_result_shape() {
        // Royalty, config, and storage domains all funnel into the same
        // standardized result type.
        let royalty: ValidationResult =
            crate::royalty_validator::validate_royalty_bps(500).into();
        let fee: ValidationResult = crate::config_validator::validate_fee(100).into();
        assert!(validate_all(&[royalty, fee]).is_ok());

        let bad_royalty: ValidationResult =
            crate::royalty_validator::validate_royalty_bps(20_000).into();
        assert_eq!(
            validate_all(&[royalty, bad_royalty, fee]),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn validate_all_stops_at_first_failure_in_order() {
        assert_eq!(
            validate_all(&[
                ValidationResult::Valid,
                ValidationResult::Invalid(Error::TokenNotFound),
                ValidationResult::Invalid(Error::Unauthorized),
            ]),
            Err(Error::TokenNotFound)
        );
    }

    #[test]
    fn validate_all_accepts_empty_and_all_valid() {
        assert!(validate_all(&[]).is_ok());
        assert!(validate_all(&[
            ValidationResult::Valid,
            ValidationResult::Valid,
        ])
        .is_ok());
    }

    #[test]
    fn standardize_bridges_result_style_validators() {
        assert!(standardize(Ok(())).is_valid());
        assert_eq!(
            standardize(Err(Error::Unauthorized)),
            ValidationResult::Invalid(Error::Unauthorized)
        );
    }

    #[test]
    fn context_aware_validator_receives_caller_and_token() {
        with_contract(|env| {
            let caller = Address::generate(env);
            let ctx = ValidationContext::new(env)
                .with_caller(&caller)
                .with_token(9);

            struct ContextEcho;
            impl Validator<()> for ContextEcho {
                fn validate(
                    &self,
                    ctx: &ValidationContext<'_>,
                    _input: &(),
                ) -> ValidationResult {
                    ValidationResult::from_condition(
                        ctx.caller.is_some() && ctx.token_id == Some(9),
                        Error::InvalidConfig,
                    )
                }
            }

            assert!(run_validator(&ContextEcho, &ctx, &()).is_ok());
        });
    }
}
