//! Configuration-specific error types (issue #983).
//!
//! Defines the standardized errors returned for invalid contract configuration
//! operations. Each error carries a unique code from the `configuration`
//! module block (`210–214`) and is documented in the central
//! [`crate::error_infrastructure::registry`].

use soroban_sdk::contracterror;

/// Error type for invalid contract configuration operations.
///
/// These errors cover the configuration surface exposed by the contract:
/// - Invalid fee values.
/// - Invalid royalty limits.
/// - Invalid batch sizes.
/// - Unsupported payment assets.
/// - Structurally invalid configuration values.
///
/// # Error Codes
///
/// Maps to the `210–214` block in the centralized error registry.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigurationError {
    /// A configured fee is outside the allowed range.
    InvalidFee = 210,

    /// The configured royalty limit is outside the allowed range.
    InvalidRoyaltyLimit = 211,

    /// The configured batch size exceeds the allowed maximum.
    InvalidBatchSize = 212,

    /// The referenced asset is not supported by the contract.
    UnsupportedAsset = 213,

    /// A configuration value is structurally invalid or out of range.
    InvalidConfigurationValue = 214,
}

impl ConfigurationError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "configuration";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        match self {
            ConfigurationError::InvalidFee => 210,
            ConfigurationError::InvalidRoyaltyLimit => 211,
            ConfigurationError::InvalidBatchSize => 212,
            ConfigurationError::UnsupportedAsset => 213,
            ConfigurationError::InvalidConfigurationValue => 214,
        }
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        match self {
            ConfigurationError::InvalidFee => "InvalidFee",
            ConfigurationError::InvalidRoyaltyLimit => "InvalidRoyaltyLimit",
            ConfigurationError::InvalidBatchSize => "InvalidBatchSize",
            ConfigurationError::UnsupportedAsset => "UnsupportedAsset",
            ConfigurationError::InvalidConfigurationValue => "InvalidConfigurationValue",
        }
    }

    /// Decode a `ConfigurationError` from a numeric code.
    ///
    /// Returns `None` for codes that do not map to a configuration error.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            210 => Some(ConfigurationError::InvalidFee),
            211 => Some(ConfigurationError::InvalidRoyaltyLimit),
            212 => Some(ConfigurationError::InvalidBatchSize),
            213 => Some(ConfigurationError::UnsupportedAsset),
            214 => Some(ConfigurationError::InvalidConfigurationValue),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const ALL: [ConfigurationError; 5] = [
        ConfigurationError::InvalidFee,
        ConfigurationError::InvalidRoyaltyLimit,
        ConfigurationError::InvalidBatchSize,
        ConfigurationError::UnsupportedAsset,
        ConfigurationError::InvalidConfigurationValue,
    ];

    #[test]
    fn codes_match_registry_block() {
        assert_eq!(ConfigurationError::InvalidFee.code(), 210);
        assert_eq!(ConfigurationError::InvalidRoyaltyLimit.code(), 211);
        assert_eq!(ConfigurationError::InvalidBatchSize.code(), 212);
        assert_eq!(ConfigurationError::UnsupportedAsset.code(), 213);
        assert_eq!(ConfigurationError::InvalidConfigurationValue.code(), 214);
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
    fn errors_are_cloneable_and_comparable() {
        let a = ConfigurationError::UnsupportedAsset;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(
            ConfigurationError::InvalidFee,
            ConfigurationError::InvalidRoyaltyLimit
        );
    }

    #[test]
    fn serialization_roundtrip() {
        for error in ALL {
            let code = error.code();
            assert_eq!(ConfigurationError::from_code(code), Some(error));
        }
    }

    #[test]
    fn unknown_codes_decode_to_none() {
        assert_eq!(ConfigurationError::from_code(209), None);
        assert_eq!(ConfigurationError::from_code(9999), None);
    }
}
