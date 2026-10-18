//! Validation pipeline (Issue #1086).
//!
//! Runs several validators against one input before a contract operation.
//!
//! # Behavior
//!
//! - Accepts any number of validators, each implementing [`Validator`]
//!   (closures `Fn(&T) -> Result<(), Error>` implement it automatically).
//! - Runs them in the order they were added.
//! - Stops at the first failure and returns that validator's [`Error`];
//!   later validators are not run.
//!
//! # Usage
//!
//! ```rust,ignore
//! ValidationPipeline::new()
//!     .add(&|uri: &String| metadata_uri_validator::validate_metadata_uri(uri))
//!     .add(&|uri: &String| metadata_size::validate_uri_size(uri))
//!     .validate(&uri)?;
//! ```

use alloc::vec::Vec;

use crate::types::Error;

/// A single validation step over an input of type `T`.
pub trait Validator<T: ?Sized> {
    /// Return `Ok(())` if `input` is valid, or the contract error describing
    /// why it is not.
    fn validate(&self, input: &T) -> Result<(), Error>;
}

impl<T: ?Sized, F> Validator<T> for F
where
    F: Fn(&T) -> Result<(), Error>,
{
    fn validate(&self, input: &T) -> Result<(), Error> {
        self(input)
    }
}

/// An ordered list of validators run against one input.
pub struct ValidationPipeline<'a, T: ?Sized> {
    validators: Vec<&'a dyn Validator<T>>,
}

impl<'a, T: ?Sized> Default for ValidationPipeline<'a, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T: ?Sized> ValidationPipeline<'a, T> {
    /// Create an empty pipeline. An empty pipeline accepts every input.
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    /// Append a validator; it runs after every validator added before it.
    pub fn add(mut self, validator: &'a dyn Validator<T>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Number of validators in the pipeline.
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// `true` when no validators have been added.
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Run every validator in order against `input`.
    ///
    /// # Errors
    /// The error of the first validator that fails.
    pub fn validate(&self, input: &T) -> Result<(), Error> {
        run_validators(input, &self.validators)
    }
}

/// Run `validators` in order against `input`, stopping at the first failure.
///
/// # Errors
/// The error of the first validator that fails.
pub fn run_validators<T: ?Sized>(
    input: &T,
    validators: &[&dyn Validator<T>],
) -> Result<(), Error> {
    for validator in validators {
        validator.validate(input)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    fn positive(v: &i128) -> Result<(), Error> {
        if *v > 0 {
            Ok(())
        } else {
            Err(Error::InvalidSalePrice)
        }
    }

    fn below_limit(v: &i128) -> Result<(), Error> {
        if *v <= 1_000 {
            Ok(())
        } else {
            Err(Error::PriceOverflow)
        }
    }

    #[test]
    fn empty_pipeline_accepts_input() {
        let pipeline: ValidationPipeline<i128> = ValidationPipeline::new();
        assert!(pipeline.is_empty());
        assert_eq!(pipeline.validate(&5), Ok(()));
    }

    #[test]
    fn accepts_multiple_validators_and_passes_valid_input() {
        let pipeline = ValidationPipeline::new().add(&positive).add(&below_limit);
        assert_eq!(pipeline.len(), 2);
        assert_eq!(pipeline.validate(&500), Ok(()));
    }

    #[test]
    fn returns_error_of_failing_validator() {
        let pipeline = ValidationPipeline::new().add(&positive).add(&below_limit);
        assert_eq!(pipeline.validate(&0), Err(Error::InvalidSalePrice));
        assert_eq!(pipeline.validate(&5_000), Err(Error::PriceOverflow));
    }

    #[test]
    fn runs_in_order_and_stops_at_first_failure() {
        let calls: RefCell<Vec<u32>> = RefCell::new(Vec::new());
        let first = |_: &i128| {
            calls.borrow_mut().push(1);
            Ok(())
        };
        let second = |_: &i128| {
            calls.borrow_mut().push(2);
            Err(Error::InvalidConfig)
        };
        let third = |_: &i128| {
            calls.borrow_mut().push(3);
            Ok(())
        };

        let result = ValidationPipeline::new()
            .add(&first)
            .add(&second)
            .add(&third)
            .validate(&1);

        assert_eq!(result, Err(Error::InvalidConfig));
        assert_eq!(*calls.borrow(), [1, 2]);
    }

    #[test]
    fn first_failure_wins_when_several_fail() {
        let fail_a = |_: &i128| Err(Error::InvalidFee);
        let fail_b = |_: &i128| Err(Error::InvalidAddress);
        let result = run_validators(&1i128, &[&fail_a, &fail_b]);
        assert_eq!(result, Err(Error::InvalidFee));
    }
}
