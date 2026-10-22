//! Operator validator (issue #1197).
//!
//! Validates NFT operators before granting or using transfer permissions.
//! An operator is a third-party address that has been approved to manage
//! tokens on behalf of an NFT owner. This validator ensures that both
//! the operator and owner addresses are valid, non-self-referential, and
//! that the operator/owner combination is structurally sound before any
//! approval is persisted.
//!
//! # Acceptance criteria (issue #1197)
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Validate operator address | [`validate_operator_address`] |
//! | Reject invalid operators | [`validate_operator`] |
//! | Prevent invalid owner/operator combinations | [`validate_operator_combination`] |
//! | Add tests | `tests` module below |
//!
//! # Errors
//!
//! - [`Error::InvalidAddress`] — operator or owner address is the zero/contract address.
//! - [`Error::OperatorNotApproved`] — operator and owner are the same address (self-approval).
//! - [`Error::Unauthorized`] — operator address is identical to the token owner (disallowed combination).
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::operator_validator::{validate_operator, OperatorInput, OperatorValidator};
//! use crate::validators::{ValidationContext, run_validator};
//!
//! // Functional API:
//! validate_operator(env, &operator, &owner)?;
//!
//! // Pipeline API:
//! let ctx = ValidationContext::new(env);
//! let input = OperatorInput { operator: operator.clone(), owner: owner.clone() };
//! run_validator(&OperatorValidator, &ctx, &input)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::types::Error;
use crate::validators::{ValidationContext, ValidationResult, Validator};

// ─── Input type ───────────────────────────────────────────────────────────────

/// Input bundle for the standardized [`OperatorValidator`].
///
/// Groups the operator and owner together so the [`Validator<OperatorInput>`]
/// interface receives a single typed value rather than separate parameters.
pub struct OperatorInput {
    /// Address proposed as the operator.
    pub operator: Address,
    /// Address of the NFT owner granting approval.
    pub owner: Address,
}

// ─── Individual validators ────────────────────────────────────────────────────

/// Validate that `operator` is a structurally valid, non-self address.
///
/// An operator address is rejected when it equals the current contract address,
/// which would allow the contract itself to act as an operator — a prohibited
/// configuration.
///
/// # Errors
/// - [`Error::InvalidAddress`] — `operator` is the current contract address.
pub fn validate_operator_address(env: &Env, operator: &Address) -> Result<(), Error> {
    if *operator == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate that `owner` is a structurally valid, non-self address.
///
/// An owner address is rejected when it equals the current contract address.
///
/// # Errors
/// - [`Error::InvalidAddress`] — `owner` is the current contract address.
pub fn validate_owner_address(env: &Env, owner: &Address) -> Result<(), Error> {
    if *owner == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate that the `operator` / `owner` combination is not self-referential.
///
/// An operator must be a distinct account from the token owner. Allowing an
/// owner to approve themselves as an operator is meaningless — the owner
/// already has full transfer rights — and can mask authorization bugs.
///
/// # Errors
/// - [`Error::OperatorNotApproved`] — `operator == owner` (self-approval).
pub fn validate_operator_combination(owner: &Address, operator: &Address) -> Result<(), Error> {
    if operator == owner {
        return Err(Error::OperatorNotApproved);
    }
    Ok(())
}

/// Run all operator validation checks in order.
///
/// This is the primary entry point. It:
/// 1. Validates the operator address (`validate_operator_address`).
/// 2. Validates the owner address (`validate_owner_address`).
/// 3. Validates the owner/operator combination (`validate_operator_combination`).
///
/// # Errors
/// - [`Error::InvalidAddress`] — operator or owner is the contract address.
/// - [`Error::OperatorNotApproved`] — operator equals owner (self-approval).
pub fn validate_operator(env: &Env, operator: &Address, owner: &Address) -> Result<(), Error> {
    validate_operator_address(env, operator)?;
    validate_owner_address(env, owner)?;
    validate_operator_combination(owner, operator)?;
    Ok(())
}

// ─── Standardized Validator<OperatorInput> implementation ────────────────────

/// Standardized operator validator implementing [`Validator<OperatorInput>`].
///
/// Wraps [`validate_operator`] so it can be composed inside validation
/// pipelines using [`crate::validators::run_validator`] and
/// [`crate::validators::validate_all`].
///
/// # Usage
/// ```rust,ignore
/// use crate::validators::{run_validator, ValidationContext};
/// use crate::operator_validator::{OperatorInput, OperatorValidator};
///
/// let ctx = ValidationContext::new(&env);
/// let input = OperatorInput { operator: op.clone(), owner: own.clone() };
/// run_validator(&OperatorValidator, &ctx, &input)?;
/// ```
pub struct OperatorValidator;

impl Validator<OperatorInput> for OperatorValidator {
    /// Validate `input.operator` and `input.owner` against all operator constraints.
    ///
    /// Returns [`ValidationResult::Valid`] when all checks pass, or
    /// [`ValidationResult::Invalid`] with the first failing error.
    fn validate(&self, ctx: &ValidationContext<'_>, input: &OperatorInput) -> ValidationResult {
        ValidationResult::from_result(validate_operator(ctx.env, &input.operator, &input.owner))
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validators::{run_validator, ValidationContext};
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

    // ── validate_operator_address ─────────────────────────────────────────────

    #[test]
    fn valid_operator_address_is_accepted() {
        with_contract(|env| {
            let operator = Address::generate(env);
            assert!(validate_operator_address(env, &operator).is_ok());
        });
    }

    #[test]
    fn contract_address_as_operator_is_rejected() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_operator_address(env, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    // ── validate_owner_address ────────────────────────────────────────────────

    #[test]
    fn valid_owner_address_is_accepted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert!(validate_owner_address(env, &owner).is_ok());
        });
    }

    #[test]
    fn contract_address_as_owner_is_rejected() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_owner_address(env, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    // ── validate_operator_combination ────────────────────────────────────────

    #[test]
    fn distinct_owner_and_operator_is_accepted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            assert!(validate_operator_combination(&owner, &operator).is_ok());
        });
    }

    #[test]
    fn self_approval_operator_equals_owner_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(
                validate_operator_combination(&owner, &owner),
                Err(Error::OperatorNotApproved)
            );
        });
    }

    // ── validate_operator (full pipeline) ────────────────────────────────────

    #[test]
    fn valid_operator_and_owner_combination_passes_all_checks() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            assert!(validate_operator(env, &operator, &owner).is_ok());
        });
    }

    #[test]
    fn operator_as_contract_address_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_operator(env, &contract_addr, &owner),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn owner_as_contract_address_is_rejected() {
        with_contract(|env| {
            let operator = Address::generate(env);
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_operator(env, &operator, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn owner_operator_same_address_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(
                validate_operator(env, &owner, &owner),
                Err(Error::OperatorNotApproved)
            );
        });
    }

    // ── OperatorValidator (Validator<OperatorInput> interface) ────────────────

    #[test]
    fn validator_interface_accepts_valid_input() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            let ctx = ValidationContext::new(env);
            let input = OperatorInput {
                operator: operator.clone(),
                owner: owner.clone(),
            };
            assert!(run_validator(&OperatorValidator, &ctx, &input).is_ok());
        });
    }

    #[test]
    fn validator_interface_rejects_self_approval() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let ctx = ValidationContext::new(env);
            let input = OperatorInput {
                operator: owner.clone(),
                owner: owner.clone(),
            };
            assert_eq!(
                run_validator(&OperatorValidator, &ctx, &input),
                Err(Error::OperatorNotApproved)
            );
        });
    }

    #[test]
    fn validator_interface_rejects_contract_as_operator() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let contract_addr = env.current_contract_address();
            let ctx = ValidationContext::new(env);
            let input = OperatorInput {
                operator: contract_addr,
                owner: owner.clone(),
            };
            assert_eq!(
                run_validator(&OperatorValidator, &ctx, &input),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn validator_interface_rejects_contract_as_owner() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            let operator = Address::generate(env);
            let ctx = ValidationContext::new(env);
            let input = OperatorInput {
                operator: operator.clone(),
                owner: contract_addr,
            };
            assert_eq!(
                run_validator(&OperatorValidator, &ctx, &input),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn multiple_different_operators_are_each_valid() {
        with_contract(|env| {
            let owner = Address::generate(env);
            for _ in 0..3 {
                let operator = Address::generate(env);
                assert!(validate_operator(env, &operator, &owner).is_ok());
            }
        });
    }
}
