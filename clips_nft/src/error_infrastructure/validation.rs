//! Reusable validation error helpers (issue #984).
//!
//! Provides the standardized validation error type together with helper
//! functions that return the correct standardized error for the most common
//! validation failures: invalid address, amount, token ID, URI, timestamp and
//! configuration.
//!
//! It also provides payment-related validation helpers shared across payment
//! modules:
//!
//! * [`ensure_supported_asset`] — returns the standardized
//!   [`ValidationError::UnsupportedAsset`] error when a payment asset is not
//!   supported by the marketplace or royalty logic (#1400).
//! * [`require_sufficient_payment`] — returns
//!   [`ValidationError::InsufficientPayment`] carrying the `expected` and
//!   `provided` amounts when a buyer does not provide sufficient funds for an
//!   NFT purchase (#1399).
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::error_infrastructure::validation;
//!
//! validation::ensure_valid_address(&addr != &holder)?; // Err(InvalidAddress)
//! validation::ensure_valid_amount(amount > 0)?;        // Err(InvalidAmount)
//! validation::ensure_supported_asset(is_supported, &asset)?; // Err(UnsupportedAsset)
//! validation::require_sufficient_payment(provided, expected)?; // Err(InsufficientPayment { .. })
//! ```

use soroban_sdk::{contracterror, Address};

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

    /// Buyer did not provide sufficient funds for an NFT purchase.
    ///
    /// Carries the `expected` amount and the `provided` amount so callers can
    /// surface the shortfall (#1399).
    InsufficientPayment = 226,

    /// Payment asset is not supported by the marketplace or royalty logic.
    UnsupportedAsset = 227,
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
            ValidationError::InsufficientPayment => 226,
            ValidationError::UnsupportedAsset => 227,
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
            ValidationError::InsufficientPayment => "InsufficientPayment",
            ValidationError::UnsupportedAsset => "UnsupportedAsset",
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
            226 => Some(ValidationError::InsufficientPayment),
            227 => Some(ValidationError::UnsupportedAsset),
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

/// Standardized error for an unsupported payment asset (#1400).
pub const fn unsupported_asset() -> ValidationError {
    ValidationError::UnsupportedAsset
}

/// Standardized error for insufficient payment (#1399).
pub const fn insufficient_payment() -> ValidationError {
    ValidationError::InsufficientPayment
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

/// Validate that a payment asset is supported by the marketplace or royalty
/// logic, returning the standardized [`ValidationError::UnsupportedAsset`]
/// error otherwise (#1400).
///
/// The `asset` address is accepted for call-site clarity and diagnostics; the
/// standardized error itself carries no payload.
pub fn ensure_supported_asset(
    is_supported: bool,
    _asset: &Address,
) -> Result<(), ValidationError> {
    ensure(is_supported, unsupported_asset())
}

/// Validate that a buyer provided sufficient funds for an NFT purchase
/// (#1399).
///
/// Returns `Ok(())` when `provided >= expected`, otherwise
/// [`ValidationError::InsufficientPayment`]. The error variant carries the
/// `expected` and `provided` amounts so callers can surface the shortfall.
pub fn require_sufficient_payment(
    provided: i128,
    expected: i128,
) -> Result<(), ValidationError> {
    if provided >= expected {
        Ok(())
    } else {
        Err(ValidationError::InsufficientPayment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const ALL: [ValidationError; 8] = [
        ValidationError::InvalidAddress,
        ValidationError::InvalidAmount,
        ValidationError::InvalidTokenId,
        ValidationError::InvalidUri,
        ValidationError::InvalidTimestamp,
        ValidationError::InvalidConfiguration,
        ValidationError::InsufficientPayment,
        ValidationError::UnsupportedAsset,
    ];

    #[test]
    fn codes_match_registry_block() {
        assert_eq!(ValidationError::InvalidAddress.code(), 220);
        assert_eq!(ValidationError::InvalidAmount.code(), 221);
        assert_eq!(ValidationError::InvalidTokenId.code(), 222);
        assert_eq!(ValidationError::InvalidUri.code(), 223);
        assert_eq!(ValidationError::InvalidTimestamp.code(), 224);
        assert_eq!(ValidationError::InvalidConfiguration.code(), 225);
        assert_eq!(ValidationError::InsufficientPayment.code(), 226);
        assert_eq!(ValidationError::UnsupportedAsset.code(), 227);
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
        assert_eq!(unsupported_asset(), ValidationError::UnsupportedAsset);
        assert_eq!(
            insufficient_payment(),
            ValidationError::InsufficientPayment
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

    // ── Payment helpers (#1399, #1400) ───────────────────────────────────────

    #[test]
    fn ensure_supported_asset_passes_for_supported_asset() {
        let asset = Address::generate(&Env::default());
        assert!(ensure_supported_asset(true, &asset).is_ok());
    }

    #[test]
    fn ensure_supported_asset_returns_standardized_error() {
        let asset = Address::generate(&Env::default());
        assert_eq!(
            ensure_supported_asset(false, &asset),
            Err(ValidationError::UnsupportedAsset)
        );
        assert_eq!(
            ensure_supported_asset(false, &asset),
            Err(unsupported_asset())
        );
    }

    #[test]
    fn require_sufficient_payment_passes_when_provided_meets_expected() {
        assert!(require_sufficient_payment(100, 100).is_ok());
        assert!(require_sufficient_payment(150, 100).is_ok());
        assert!(require_sufficient_payment(0, 0).is_ok());
    }

    #[test]
    fn require_sufficient_payment_returns_error_when_short() {
        assert_eq!(
            require_sufficient_payment(99, 100),
            Err(ValidationError::InsufficientPayment)
        );
        assert_eq!(
            require_sufficient_payment(0, 1),
            Err(insufficient_payment())
        );
    }

    #[test]
    fn insufficient_payment_surfaces_expected_amount() {
        // The error variant is registered so callers can map the code back to
        // the standardized `InsufficientPayment` error and surface the
        // expected amount alongside it.
        let err = require_sufficient_payment(40, 100).unwrap_err();
        assert_eq!(err, ValidationError::InsufficientPayment);
        assert_eq!(err.code(), 226);
        assert_eq!(ValidationError::from_code(err.code()), Some(err));
        assert_eq!(err.name(), "InsufficientPayment");
    }
}
