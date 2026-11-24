//! Configuration-specific error types (issue #1378).
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
/// - Invalid addresses.
/// - Invalid URIs.
/// - Invalid limits.
/// - Unauthorized configuration updates.
///
/// # Error Codes
///
/// Maps to the `210–218` block in the centralized error registry.
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

    /// A configured address is invalid (e.g. contract address or blacklisted).
    InvalidAddress = 215,

    /// A configured URI is empty or uses an unsupported protocol.
    InvalidURI = 216,

    /// A configured limit (e.g. collection limit or version) is out of range.
    InvalidLimit = 217,

    /// Caller is not authorized to update contract configuration.
    UnauthorizedConfigurationUpdate = 218,
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
            ConfigurationError::InvalidAddress => 215,
            ConfigurationError::InvalidURI => 216,
            ConfigurationError::InvalidLimit => 217,
            ConfigurationError::UnauthorizedConfigurationUpdate => 218,
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
            ConfigurationError::InvalidAddress => "InvalidAddress",
            ConfigurationError::InvalidURI => "InvalidURI",
            ConfigurationError::InvalidLimit => "InvalidLimit",
            ConfigurationError::UnauthorizedConfigurationUpdate => {
                "UnauthorizedConfigurationUpdate"
            }
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
            215 => Some(ConfigurationError::InvalidAddress),
            216 => Some(ConfigurationError::InvalidURI),
            217 => Some(ConfigurationError::InvalidLimit),
            218 => Some(ConfigurationError::UnauthorizedConfigurationUpdate),
            _ => None,
        }
    }

    /// Convert into the top-level contract [`crate::types::Error`].
    pub const fn to_error(self) -> crate::types::Error {
        match self {
            ConfigurationError::InvalidFee => crate::types::Error::InvalidFee,
            ConfigurationError::InvalidRoyaltyLimit => crate::types::Error::InvalidBasisPoints,
            ConfigurationError::InvalidBatchSize => crate::types::Error::InvalidConfig,
            ConfigurationError::UnsupportedAsset => crate::types::Error::UnsupportedAsset,
            ConfigurationError::InvalidConfigurationValue => crate::types::Error::InvalidConfig,
            ConfigurationError::InvalidAddress => crate::types::Error::InvalidAddress,
            ConfigurationError::InvalidURI => crate::types::Error::InvalidURI,
            ConfigurationError::InvalidLimit => crate::types::Error::InvalidLimit,
            ConfigurationError::UnauthorizedConfigurationUpdate => {
                crate::types::Error::UnauthorizedConfigurationUpdate
            }
        }
    }
}

impl From<ConfigurationError> for crate::types::Error {
    fn from(err: ConfigurationError) -> Self {
        err.to_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const ALL: [ConfigurationError; 9] = [
        ConfigurationError::InvalidFee,
        ConfigurationError::InvalidRoyaltyLimit,
        ConfigurationError::InvalidBatchSize,
        ConfigurationError::UnsupportedAsset,
        ConfigurationError::InvalidConfigurationValue,
        ConfigurationError::InvalidAddress,
        ConfigurationError::InvalidURI,
        ConfigurationError::InvalidLimit,
        ConfigurationError::UnauthorizedConfigurationUpdate,
    ];

    #[test]
    fn codes_match_registry_block()
    {
        assert_eq!(ConfigurationError::InvalidFee.code(), 210);
        assert_eq!(ConfigurationError::InvalidRoyaltyLimit.code(), 211);
        assert_eq!(ConfigurationError::InvalidBatchSize.code(), 212);
        assert_eq!(ConfigurationError::UnsupportedAsset.code(), 213);
        assert_eq!(ConfigurationError::InvalidConfigurationValue.code(), 214);
        assert_eq!(ConfigurationError::InvalidAddress.code(), 215);
        assert_eq!(ConfigurationError::InvalidURI.code(), 216);
        assert_eq!(ConfigurationError::InvalidLimit.code(), 217);
        assert_eq!(
            ConfigurationError::UnauthorizedConfigurationUpdate.code(),
            218
        );
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
        assert_eq!(ConfigurationError::from_code(219), None);
        assert_eq!(ConfigurationError::from_code(9999), None);
    }

    #[test]
    fn converts_to_contract_error() {
        assert_eq!(
            crate::types::Error::from(ConfigurationError::InvalidFee),
            crate::types::Error::InvalidFee
        );
        assert_eq!(
            crate::types::Error::from(ConfigurationError::InvalidAddress),
            crate::types::Error::InvalidAddress
        );
        assert_eq!(
            crate::types::Error::from(ConfigurationError::InvalidURI),
            crate::types::Error::InvalidURI
        );
        assert_eq!(
            crate::types::Error::from(ConfigurationError::InvalidLimit),
            crate::types::Error::InvalidLimit
        );
        assert_eq!(
            crate::types::Error::from(ConfigurationError::UnauthorizedConfigurationUpdate),
            crate::types::Error::UnauthorizedConfigurationUpdate
        );
    }
}
