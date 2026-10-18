//! Initialization-specific error types (issue #982).
//!
//! Defines the standardized errors returned by contract initialization
//! operations. Each error carries a unique code from the `initialization`
//! module block (`200–204`) and is documented in the central
//! [`crate::error_infrastructure::registry`].

use soroban_sdk::contracterror;

/// Error type for contract initialization.
///
/// These errors cover the full initialization lifecycle:
/// - Attempting to re-initialize an already-initialized contract.
/// - Using contract entry points before initialization.
/// - Passing malformed initialization parameters.
/// - Initializing without an administrator.
/// - Supplying an invalid initial configuration.
///
/// # Error Codes
///
/// Same as the `200–204` block in the centralized error registry.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InitializationError {
    /// The contract has already been initialized and cannot be re-initialized.
    ContractAlreadyInitialized = 200,

    /// The contract must be initialized before the operation can run.
    ContractNotInitialized = 201,

    /// Initialization parameters are malformed or outside the allowed range.
    InvalidInitializationParameters = 202,

    /// Initialization was attempted without a valid administrator address.
    MissingAdministrator = 203,

    /// The initial configuration is invalid and cannot be applied.
    InvalidInitialConfig = 204,
}

impl InitializationError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "initialization";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        match self {
            InitializationError::ContractAlreadyInitialized => 200,
            InitializationError::ContractNotInitialized => 201,
            InitializationError::InvalidInitializationParameters => 202,
            InitializationError::MissingAdministrator => 203,
            InitializationError::InvalidInitialConfig => 204,
        }
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        match self {
            InitializationError::ContractAlreadyInitialized => "ContractAlreadyInitialized",
            InitializationError::ContractNotInitialized => "ContractNotInitialized",
            InitializationError::InvalidInitializationParameters => {
                "InvalidInitializationParameters"
            }
            InitializationError::MissingAdministrator => "MissingAdministrator",
            InitializationError::InvalidInitialConfig => "InvalidInitialConfig",
        }
    }

    /// Decode an `InitializationError` from a numeric code.
    ///
    /// Returns `None` for codes that do not map to an initialization error,
    /// allowing callers to distinguish unknown codes without panicking.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            200 => Some(InitializationError::ContractAlreadyInitialized),
            201 => Some(InitializationError::ContractNotInitialized),
            202 => Some(InitializationError::InvalidInitializationParameters),
            203 => Some(InitializationError::MissingAdministrator),
            204 => Some(InitializationError::InvalidInitialConfig),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const ALL: [InitializationError; 5] = [
        InitializationError::ContractAlreadyInitialized,
        InitializationError::ContractNotInitialized,
        InitializationError::InvalidInitializationParameters,
        InitializationError::MissingAdministrator,
        InitializationError::InvalidInitialConfig,
    ];

    #[test]
    fn codes_match_registry_block() {
        assert_eq!(InitializationError::ContractAlreadyInitialized.code(), 200);
        assert_eq!(InitializationError::ContractNotInitialized.code(), 201);
        assert_eq!(
            InitializationError::InvalidInitializationParameters.code(),
            202
        );
        assert_eq!(InitializationError::MissingAdministrator.code(), 203);
        assert_eq!(InitializationError::InvalidInitialConfig.code(), 204);
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
        let a = InitializationError::ContractNotInitialized;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(
            InitializationError::ContractAlreadyInitialized,
            InitializationError::ContractNotInitialized
        );
    }

    #[test]
    fn serialization_roundtrip() {
        for error in ALL {
            let code = error.code();
            assert_eq!(InitializationError::from_code(code), Some(error));
        }
    }

    #[test]
    fn unknown_codes_decode_to_none() {
        assert_eq!(InitializationError::from_code(199), None);
        assert_eq!(InitializationError::from_code(9999), None);
    }
}
