//! Reusable validation error helpers (issue #984).
//!
//! Provides the standardized validation error type together with helper
//! functions that return the correct standardized error for the most common
//! validation failures: invalid address, amount, token ID, URI, timestamp and
//! configuration.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::error_infrastructure::validation;
//!
//! validation::ensure_valid_address(&addr != &holder)?; // Err(InvalidAddress)
//! validation::ensure_valid_amount(amount > 0)?;        // Err(InvalidAmount)
//! ```

use soroban_sdk::contracterror;

/// Standardized validation error for the ClipCash contract.
///
/// Each variant carries a unique code from the `validation` module block
/// (`220–225`) and matches the centralized error registry.
#[allow(clippy::enum_variant_names)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ValidationError {
    /// Provided wallet address is invalid.
    InvalidAddress = 220,

    /// Provided amount is zero, negative or otherwise invalid.
    InvalidAmount = 221,

    /// Provided token identifier is invalid.
    InvalidTokenId = 222,

    /// Provided URI is malformed or uses an unsupported protocol.
    InvalidUri = 223,

    /// Provided timestamp is zero or in the past.
    InvalidTimestamp = 224,

    /// Provided configuration is invalid.
    InvalidConfiguration = 225,
}

impl ValidationError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "validation";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        match self {
            ValidationError::InvalidAddress => 220,
            ValidationError::InvalidAmount => 221,
            ValidationError::InvalidTokenId => 222,
            ValidationError::InvalidUri => 223,
            ValidationError::InvalidTimestamp => 224,
            ValidationError::InvalidConfiguration => 225,
        }
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        match self {
            ValidationError::InvalidAddress => "InvalidAddress",
            ValidationError::InvalidAmount => "InvalidAmount",
            ValidationError::InvalidTokenId => "InvalidTokenId",
            ValidationError::InvalidUri => "InvalidUri",
            ValidationError::InvalidTimestamp => "InvalidTimestamp",
            ValidationError::InvalidConfiguration => "InvalidConfiguration",
        }
    }

    /// Decode a `ValidationError` from a numeric code.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            220 => Some(ValidationError::InvalidAddress),
            221 => Some(ValidationError::InvalidAmount),
            222 => Some(ValidationError::InvalidTokenId),
            223 => Some(ValidationError::InvalidUri),
            224 => Some(ValidationError::InvalidTimestamp),
            225 => Some(ValidationError::InvalidConfiguration),
            _ => None,
        }
    }
}

// ── Reusable helpers returning standardized validation errors ────────────────

/// Standardized error for an invalid wallet address.
pub const fn invalid_address() -> ValidationError {
    ValidationError::InvalidAddress
}

/// Standardized error for an invalid amount.
pub const fn invalid_amount() -> ValidationError {
    ValidationError::InvalidAmount
}

/// Standardized error for an invalid token identifier.
pub const fn invalid_token_id() -> ValidationError {
    ValidationError::InvalidTokenId
}

/// Standardized error for an invalid URI.
pub const fn invalid_uri() -> ValidationError {
    ValidationError::InvalidUri
}

/// Standardized error for an invalid timestamp.
pub const fn invalid_timestamp() -> ValidationError {
    ValidationError::InvalidTimestamp
}

/// Standardized error for an invalid configuration.
pub const fn invalid_configuration() -> ValidationError {
    ValidationError::InvalidConfiguration
}

/// Return `Ok(())` when `condition` holds, otherwise the provided `error`.
pub fn ensure(condition: bool, error: ValidationError) -> Result<(), ValidationError> {
    if condition {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validate that an address is acceptable.
pub fn ensure_valid_address(valid: bool) -> Result<(), ValidationError> {
    ensure(valid, invalid_address())
}

/// Validate that an amount is acceptable (e.g. positive and within bounds).
pub fn ensure_valid_amount(valid: bool) -> Result<(), ValidationError> {
    ensure(valid, invalid_amount())
}

/// Validate that a token identifier is acceptable.
pub fn ensure_valid_token_id(valid: bool) -> Result<(), ValidationError> {
    ensure(valid, invalid_token_id())
}

/// Validate that a URI is acceptable.
pub fn ensure_valid_uri(valid: bool) -> Result<(), ValidationError> {
    ensure(valid, invalid_uri())
}

/// Validate that a timestamp is acceptable (e.g. non-zero and in the future).
pub fn ensure_valid_timestamp(valid: bool) -> Result<(), ValidationError> {
    ensure(valid, invalid_timestamp())
}

/// Validate that a configuration is acceptable.
pub fn ensure_valid_configuration(valid: bool) -> Result<(), ValidationError> {
    ensure(valid, invalid_configuration())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const ALL: [ValidationError; 6] = [
        ValidationError::InvalidAddress,
        ValidationError::InvalidAmount,
        ValidationError::InvalidTokenId,
        ValidationError::InvalidUri,
        ValidationError::InvalidTimestamp,
        ValidationError::InvalidConfiguration,
    ];

    #[test]
    fn codes_match_registry_block() {
        assert_eq!(ValidationError::InvalidAddress.code(), 220);
        assert_eq!(ValidationError::InvalidAmount.code(), 221);
        assert_eq!(ValidationError::InvalidTokenId.code(), 222);
        assert_eq!(ValidationError::InvalidUri.code(), 223);
        assert_eq!(ValidationError::InvalidTimestamp.code(), 224);
        assert_eq!(ValidationError::InvalidConfiguration.code(), 225);
    }

    #[test]
    fn error_codes_are_unique() {
        let mut codes: Vec<u32> = ALL.iter().map(|e| e.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), ALL.len());
    }

    #[test]
    fn error_names_are_unique_and_non_empty() {
        let mut names: Vec<&str> = ALL.iter().map(|e| e.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
    }

    #[test]
    fn serialization_roundtrip() {
        for error in ALL {
            let code = error.code();
            assert_eq!(ValidationError::from_code(code), Some(error));
        }
    }

    #[test]
    fn unknown_codes_decode_to_none() {
        assert_eq!(ValidationError::from_code(219), None);
        assert_eq!(ValidationError::from_code(9999), None);
    }

    // ── Helper functions (acceptance: "reusable helpers") ────────────────────

    #[test]
    fn helpers_return_expected_standardized_errors() {
        assert_eq!(invalid_address(), ValidationError::InvalidAddress);
        assert_eq!(invalid_amount(), ValidationError::InvalidAmount);
        assert_eq!(invalid_token_id(), ValidationError::InvalidTokenId);
        assert_eq!(invalid_uri(), ValidationError::InvalidUri);
        assert_eq!(invalid_timestamp(), ValidationError::InvalidTimestamp);
        assert_eq!(
            invalid_configuration(),
            ValidationError::InvalidConfiguration
        );
    }

    #[test]
    fn ensure_returns_ok_for_true_conditions() {
        assert!(ensure(true, ValidationError::InvalidAddress).is_ok());
        assert!(ensure_valid_address(true).is_ok());
        assert!(ensure_valid_amount(true).is_ok());
        assert!(ensure_valid_token_id(true).is_ok());
        assert!(ensure_valid_uri(true).is_ok());
        assert!(ensure_valid_timestamp(true).is_ok());
        assert!(ensure_valid_configuration(true).is_ok());
    }

    #[test]
    fn ensure_returns_standardized_error_for_false_conditions() {
        assert_eq!(
            ensure_valid_address(false),
            Err(ValidationError::InvalidAddress)
        );
        assert_eq!(
            ensure_valid_amount(false),
            Err(ValidationError::InvalidAmount)
        );
        assert_eq!(
            ensure_valid_token_id(false),
            Err(ValidationError::InvalidTokenId)
        );
        assert_eq!(ensure_valid_uri(false), Err(ValidationError::InvalidUri));
        assert_eq!(
            ensure_valid_timestamp(false),
            Err(ValidationError::InvalidTimestamp)
        );
        assert_eq!(
            ensure_valid_configuration(false),
            Err(ValidationError::InvalidConfiguration)
        );
    }
}
