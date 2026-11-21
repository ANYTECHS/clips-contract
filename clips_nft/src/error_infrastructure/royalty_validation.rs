//! Standardized royalty validation errors (issue #1092).
//!
//! This module defines machine-readable errors for detecting and reporting
//! invalid royalty configurations, including invalid basis points, recipient
//! addresses, and state transitions.
//!
//! # Error code allocation
//!
//! Royalty validation errors are assigned codes in the range **260–265**:
//!
//! | Code | Error | Meaning |
//! |------|-------|---------|
//! | 260 | `InvalidRoyaltyBps` | Royalty basis points value is invalid |
//! | 261 | `InvalidRoyaltyRecipient` | Recipient address is invalid or unauthorized |
//! | 262 | `InvalidRoyaltyState` | Royalty state transition is invalid |
//! | 263 | `UnauthorizedRoyaltyUpdate` | Caller lacks permissions to update royalty |
//! | 264 | `RoyaltyAboveMaximum` | Royalty exceeds the maximum allowed royalty |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::error_infrastructure::royalty_validation::RoyaltyValidationError;
//!
//! if bps > MAX_ROYALTY_BPS {
//!     return Err(RoyaltyValidationError::InvalidRoyaltyBps.into());
//! }
//! ```

use soroban_sdk::contracterror;

use crate::types::Error;

/// Standardized errors for royalty configuration validation (issue #1092).
///
/// Each error corresponds to a specific validation failure during royalty
/// configuration, update, or state transition operations.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoyaltyValidationError {
    /// Royalty basis points value is invalid (code 260).
    ///
    /// Returned when:
    /// - A provided royalty bps is malformed or out of the valid `0..=10000` range
    /// - The total of royalty + platform fees exceeds 100%
    InvalidRoyaltyBps = 260,

    /// Royalty recipient address is invalid or unauthorized (code 261).
    ///
    /// Returned when:
    /// - The recipient address is empty or `0x0`
    /// - The recipient is in a blacklist or blocked addresses list
    /// - The recipient fails cross-chain address validation
    InvalidRoyaltyRecipient = 261,

    /// Royalty state transition is invalid (code 262).
    ///
    /// Returned when:
    /// - An attempt is made to update a frozen royalty configuration
    /// - An attempt is made to freeze an already-frozen configuration
    /// - An attempt is made to remove a non-existent royalty
    InvalidRoyaltyState = 262,

    /// Caller is not authorized to update the royalty (code 263).
    ///
    /// Returned when:
    /// - A non-admin attempts to update token-level royalty
    /// - A non-owner attempts to update their own earnings
    /// - The caller lacks a required signature for the operation
    UnauthorizedRoyaltyUpdate = 263,

    /// Royalty exceeds the maximum allowed royalty (code 264).
    ///
    /// Returned when:
    /// - A provided royalty bps is greater than [`MAX_ROYALTY_BPS`](crate::storage_constants::MAX_ROYALTY_BPS)
    /// - The resulting royalty configuration would exceed the protocol maximum
    RoyaltyAboveMaximum = 264,
}

impl RoyaltyValidationError {
    /// Maps this royalty validation error to the standardized [`Error`] type.
    ///
    /// This allows callers to return royalty validation failures through the
    /// contract's unified error surface.
    pub fn to_error(self) -> Error {
        match self {
            RoyaltyValidationError::InvalidRoyaltyBps => Error::InvalidRoyaltyBps,
            RoyaltyValidationError::InvalidRoyaltyRecipient => Error::InvalidRecipient,
            RoyaltyValidationError::InvalidRoyaltyState => Error::InvalidRoyaltyState,
            RoyaltyValidationError::UnauthorizedRoyaltyUpdate => Error::Unauthorized,
            RoyaltyValidationError::RoyaltyAboveMaximum => Error::RoyaltyTooHigh,
        }
    }
}

impl From<RoyaltyValidationError> for Error {
    fn from(err: RoyaltyValidationError) -> Self {
        err.to_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_unique() {
        let invalid_bps = RoyaltyValidationError::InvalidRoyaltyBps as u32;
        let invalid_recipient = RoyaltyValidationError::InvalidRoyaltyRecipient as u32;
        let invalid_state = RoyaltyValidationError::InvalidRoyaltyState as u32;
        let unauthorized = RoyaltyValidationError::UnauthorizedRoyaltyUpdate as u32;
        let above_maximum = RoyaltyValidationError::RoyaltyAboveMaximum as u32;

        assert_eq!(invalid_bps, 260);
        assert_eq!(invalid_recipient, 261);
        assert_eq!(invalid_state, 262);
        assert_eq!(unauthorized, 263);
        assert_eq!(above_maximum, 264);

        let codes = [
            invalid_bps,
            invalid_recipient,
            invalid_state,
            unauthorized,
            above_maximum,
        ];
        for (i, a) in codes.iter().enumerate() {
            for b in codes.iter().skip(i + 1) {
                assert_ne!(a, b, "error codes must be unique");
            }
        }
    }

    #[test]
    fn error_codes_are_in_correct_range() {
        for code in 260..=264u32 {
            let error = match code {
                260 => Some(RoyaltyValidationError::InvalidRoyaltyBps),
                261 => Some(RoyaltyValidationError::InvalidRoyaltyRecipient),
                262 => Some(RoyaltyValidationError::InvalidRoyaltyState),
                263 => Some(RoyaltyValidationError::UnauthorizedRoyaltyUpdate),
                264 => Some(RoyaltyValidationError::RoyaltyAboveMaximum),
                _ => None,
            };
            assert!(error.is_some(), "code {} should be defined", code);
        }
    }

    #[test]
    fn maps_to_standardized_errors() {
        assert_eq!(
            RoyaltyValidationError::InvalidRoyaltyBps.to_error(),
            Error::InvalidRoyaltyBps
        );
        assert_eq!(
            RoyaltyValidationError::InvalidRoyaltyRecipient.to_error(),
            Error::InvalidRecipient
        );
        assert_eq!(
            RoyaltyValidationError::InvalidRoyaltyState.to_error(),
            Error::InvalidRoyaltyState
        );
        assert_eq!(
            RoyaltyValidationError::UnauthorizedRoyaltyUpdate.to_error(),
            Error::Unauthorized
        );
        assert_eq!(
            RoyaltyValidationError::RoyaltyAboveMaximum.to_error(),
            Error::RoyaltyTooHigh
        );
    }

    #[test]
    fn converts_into_standardized_error() {
        let err: Error = RoyaltyValidationError::RoyaltyAboveMaximum.into();
        assert_eq!(err, Error::RoyaltyTooHigh);
    }
}
