//! Error infrastructure tests (issue #986).
//!
//! Verifies that the centralized error infrastructure works consistently:
//!
//! * **Unique error codes** — no two standardized errors share a code.
//! * **Error categories** — categories serialize/deserialize and classify
//!   codes as expected.
//! * **Error names** — every error exposes a stable machine-readable name.
//! * **Error helper functions** — ownership/existence guards behave correctly.
//! * **Error serialization** — code ⇄ error round-trips are lossless.

use alloc::vec::Vec;

use crate::error_catalog::{
    categorize_by_code, ensure_owner, ensure_token_exists, ensure_valid_configuration,
    ensure_valid_validation, invalid_configuration, invalid_validation, require_owner,
    require_token_exists, ConfigurationError, ErrorCategory, TokenNotFoundError,
    UnauthorizedOwnerError, ValidationError,
    categorize_by_code, ensure_owner, ensure_token_exists, require_owner, require_token_exists,
    ErrorCategory, InitializationError, TokenNotFoundError, UnauthorizedOwnerError,
    ErrorCategory, InsufficientPaymentError, TokenNotFoundError, UnauthorizedOwnerError,
    UnsupportedAssetError,
    ApprovalAlreadyExistsError, ErrorCategory, InvalidRecipientError, InvalidTransferStateError,
    TokenNotFoundError, UnauthorizedOwnerError,
};

/// Every standardized error produced by the error infrastructure.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum StandardizedError {
    Configuration(ConfigurationError),
    Validation(ValidationError),
    TokenNotFound(TokenNotFoundError),
    UnauthorizedOwner(UnauthorizedOwnerError),
    Initialization(InitializationError),
    InsufficientPayment(InsufficientPaymentError),
    UnsupportedAsset(UnsupportedAssetError),
    InvalidRecipient(InvalidRecipientError),
    ApprovalAlreadyExists(ApprovalAlreadyExistsError),
    InvalidTransferState(InvalidTransferStateError),
}

impl StandardizedError {
    const fn code(self) -> u32 {
        match self {
            StandardizedError::Configuration(e) => e.code(),
            StandardizedError::Validation(e) => e.code(),
            StandardizedError::TokenNotFound(e) => e.code(),
            StandardizedError::UnauthorizedOwner(e) => e.code(),
            StandardizedError::Initialization(e) => e.code(),
            StandardizedError::InsufficientPayment(e) => e.code(),
            StandardizedError::UnsupportedAsset(e) => e.code(),
            StandardizedError::InvalidRecipient(e) => e.code(),
            StandardizedError::ApprovalAlreadyExists(e) => e.code(),
            StandardizedError::InvalidTransferState(e) => e.code(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            StandardizedError::Configuration(e) => e.name(),
            StandardizedError::Validation(e) => e.name(),
            StandardizedError::TokenNotFound(e) => e.name(),
            StandardizedError::UnauthorizedOwner(e) => e.name(),
            StandardizedError::Initialization(e) => e.name(),
        }
    }
}

const ALL_INITIALIZATION_ERRORS: [InitializationError; 5] = [
    InitializationError::AlreadyInitialized,
    InitializationError::NotInitialized,
    InitializationError::InvalidInitializationParameters,
    InitializationError::MissingAdministrator,
    InitializationError::InvalidInitialConfiguration,
];

const ALL_STANDARDIZED: [StandardizedError; 7] = [
    StandardizedError::TokenNotFound(TokenNotFoundError::TokenNotFound),
    StandardizedError::UnauthorizedOwner(UnauthorizedOwnerError::UnauthorizedOwner),
    StandardizedError::Initialization(InitializationError::AlreadyInitialized),
    StandardizedError::Initialization(InitializationError::NotInitialized),
    StandardizedError::Initialization(InitializationError::InvalidInitializationParameters),
    StandardizedError::Initialization(InitializationError::MissingAdministrator),
    StandardizedError::Initialization(InitializationError::InvalidInitialConfiguration),
            StandardizedError::InsufficientPayment(e) => e.name(),
            StandardizedError::UnsupportedAsset(e) => e.name(),
            StandardizedError::InvalidRecipient(e) => e.name(),
            StandardizedError::ApprovalAlreadyExists(e) => e.name(),
            StandardizedError::InvalidTransferState(e) => e.name(),
        }
    }
}

const ALL_STANDARDIZED: [StandardizedError; 4] = [
    StandardizedError::TokenNotFound(TokenNotFoundError::TokenNotFound),
    StandardizedError::UnauthorizedOwner(UnauthorizedOwnerError::UnauthorizedOwner),
    StandardizedError::InsufficientPayment(InsufficientPaymentError::InsufficientPayment),
    StandardizedError::UnsupportedAsset(UnsupportedAssetError::UnsupportedAsset),
    StandardizedError::Configuration(ConfigurationError::InvalidConfiguration),
    StandardizedError::Validation(ValidationError::InvalidValidation),
const ALL_STANDARDIZED: [StandardizedError; 9] = [
    StandardizedError::TokenNotFound(TokenNotFoundError::TokenNotFound),
    StandardizedError::UnauthorizedOwner(UnauthorizedOwnerError::UnauthorizedOwner),
    StandardizedError::InvalidRecipient(InvalidRecipientError::InvalidRecipient),
    StandardizedError::ApprovalAlreadyExists(ApprovalAlreadyExistsError::ApprovalAlreadyExists),
    StandardizedError::InvalidTransferState(InvalidTransferStateError::Burned),
    StandardizedError::InvalidTransferState(InvalidTransferStateError::Frozen),
    StandardizedError::InvalidTransferState(InvalidTransferStateError::MissingOwner),
    StandardizedError::InvalidTransferState(InvalidTransferStateError::InvalidOwnershipState),
    StandardizedError::InvalidTransferState(InvalidTransferStateError::InactiveToken),
];

// ── Unique error codes ────────────────────────────────────────────────────────

#[test]
fn standardized_error_codes_are_unique() {
    let mut codes: Vec<u32> = ALL_STANDARDIZED.iter().map(|e| e.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), ALL_STANDARDIZED.len());
}

#[test]
fn standardized_error_names_are_unique() {
    let mut names: Vec<&str> = ALL_STANDARDIZED.iter().map(|e| e.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), ALL_STANDARDIZED.len());
}

// ── Error categories ──────────────────────────────────────────────────────────

#[test]
fn every_standardized_error_maps_to_a_category() {
    for error in ALL_STANDARDIZED {
        assert!(
            categorize_by_code(error.code()).is_some(),
            "{} has no category",
            error.name()
        );
    }
}

#[test]
fn categories_classify_codes() {
    assert_eq!(
        categorize_by_code(ConfigurationError::CODE),
        Some(ErrorCategory::Configuration)
    );
    assert_eq!(
        categorize_by_code(ValidationError::CODE),
        Some(ErrorCategory::Validation)
    );
    assert_eq!(
        categorize_by_code(TokenNotFoundError::CODE),
        Some(ErrorCategory::Storage)
    );
    assert_eq!(
        categorize_by_code(UnauthorizedOwnerError::CODE),
        Some(ErrorCategory::Ownership)
    );
    assert_eq!(
        categorize_by_code(InsufficientPaymentError::CODE),
        Some(ErrorCategory::Payment)
    );
    assert_eq!(
        categorize_by_code(UnsupportedAssetError::CODE),
        Some(ErrorCategory::Payment)
        categorize_by_code(InvalidRecipientError::CODE),
        Some(ErrorCategory::Validation)
    );
    assert_eq!(
        categorize_by_code(ApprovalAlreadyExistsError::CODE),
        Some(ErrorCategory::Approval)
    );
    assert_eq!(
        categorize_by_code(InvalidTransferStateError::CODE),
        Some(ErrorCategory::Transfer)
    );
}

#[test]
fn token_not_found_has_expected_code_and_category() {
    assert_eq!(TokenNotFoundError::CODE, 230);
    assert_eq!(TokenNotFoundError::TokenNotFound.code(), 230);
    assert_eq!(
        categorize_by_code(TokenNotFoundError::CODE),
        Some(ErrorCategory::Storage)
    );
}

#[test]
fn category_names_are_stable() {
    for category in crate::error_catalog::categories::ALL_CATEGORIES {
        assert_eq!(
            ErrorCategory::from_code(category.code()),
            Some(category),
            "category {} fails round-trip",
            category.as_str()
        );
    }
}

#[test]
fn all_nine_categories_are_present() {
    let all = crate::error_catalog::categories::ALL_CATEGORIES;
    assert_eq!(all.len(), 9);

    let expected = [
        ErrorCategory::Validation,
        ErrorCategory::Authorization,
        ErrorCategory::Ownership,
        ErrorCategory::Payment,
        ErrorCategory::Marketplace,
        ErrorCategory::Royalty,
        ErrorCategory::Storage,
        ErrorCategory::Security,
        ErrorCategory::Configuration,
    ];
    for category in expected {
        assert!(
            all.contains(&category),
            "missing category {}",
            category.as_str()
        );
    }
}

#[test]
fn category_codes_are_unique() {
    let mut codes: Vec<u32> = crate::error_catalog::categories::ALL_CATEGORIES
        .iter()
        .map(|c| c.code())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(
        codes.len(),
        crate::error_catalog::categories::ALL_CATEGORIES.len()
    );
}

#[test]
fn category_codes_roundtrip_through_from_code() {
    for category in crate::error_catalog::categories::ALL_CATEGORIES {
        assert_eq!(ErrorCategory::from_code(category.code()), Some(category));
    }
}

// ── Initialization errors ─────────────────────────────────────────────────────

#[test]
fn initialization_errors_map_to_configuration_category() {
    for error in ALL_INITIALIZATION_ERRORS {
        assert_eq!(
            categorize_by_code(error.code()),
            Some(ErrorCategory::Configuration),
            "{} is not classified as Configuration",
            error.name()
        );
    }
}

#[test]
fn initialization_error_names_are_stable() {
    assert_eq!(InitializationError::AlreadyInitialized.name(), "AlreadyInitialized");
    assert_eq!(InitializationError::NotInitialized.name(), "NotInitialized");
    assert_eq!(
        InitializationError::InvalidInitializationParameters.name(),
        "InvalidInitializationParameters"
    );
    assert_eq!(
        InitializationError::MissingAdministrator.name(),
        "MissingAdministrator"
    );
    assert_eq!(
        InitializationError::InvalidInitialConfiguration.name(),
        "InvalidInitialConfiguration"
    );
}

// ── Ownership errors ──────────────────────────────────────────────────────────

#[test]
fn unauthorized_owner_is_classified_as_ownership() {
    assert_eq!(
        categorize_by_code(UnauthorizedOwnerError::CODE),
        Some(ErrorCategory::Ownership)
    );
}

#[test]
fn ownership_helpers_return_unauthorized_owner_error() {
    assert_eq!(
        ensure_owner(false),
        Err(UnauthorizedOwnerError::UnauthorizedOwner)
    );
    assert!(ensure_owner(true).is_ok());
}

// ── Error names ───────────────────────────────────────────────────────────────

#[test]
fn error_names_match_discriminants() {
    assert_eq!(ConfigurationError::NAME, "InvalidConfiguration");
    assert_eq!(ValidationError::NAME, "InvalidValidation");
    assert_eq!(TokenNotFoundError::NAME, "TokenNotFound");
    assert_eq!(UnauthorizedOwnerError::NAME, "UnauthorizedOwner");
    assert_eq!(InsufficientPaymentError::NAME, "InsufficientPayment");
    assert_eq!(UnsupportedAssetError::NAME, "UnsupportedAsset");
    assert_eq!(InvalidRecipientError::NAME, "InvalidRecipient");
    assert_eq!(ApprovalAlreadyExistsError::NAME, "ApprovalAlreadyExists");
    assert_eq!(InvalidTransferStateError::NAME, "InvalidTransferState");
    for error in ALL_STANDARDIZED {
        assert!(!error.name().is_empty());
        let _ = error.code();
    }
}

// ── Per-error code / name / round-trip tests ─────────────────────────────────

#[test]
fn invalid_recipient_error_roundtrip() {
    let error = InvalidRecipientError::InvalidRecipient;
    assert_eq!(error.code(), InvalidRecipientError::CODE);
    assert_eq!(error.name(), InvalidRecipientError::NAME);
    assert_eq!(
        InvalidRecipientError::from_code(error.code()),
        Some(error)
    );
    assert_eq!(
        categorize_by_code(error.code()),
        Some(ErrorCategory::Validation)
    );
}

#[test]
fn approval_already_exists_error_roundtrip() {
    let error = ApprovalAlreadyExistsError::ApprovalAlreadyExists;
    assert_eq!(error.code(), ApprovalAlreadyExistsError::CODE);
    assert_eq!(error.name(), ApprovalAlreadyExistsError::NAME);
    assert_eq!(
        ApprovalAlreadyExistsError::from_code(error.code()),
        Some(error)
    );
    assert_eq!(
        categorize_by_code(error.code()),
        Some(ErrorCategory::Approval)
    );
}

#[test]
fn invalid_transfer_state_error_roundtrip() {
    let reasons = [
        InvalidTransferStateError::Burned,
        InvalidTransferStateError::Frozen,
        InvalidTransferStateError::MissingOwner,
        InvalidTransferStateError::InvalidOwnershipState,
        InvalidTransferStateError::InactiveToken,
    ];
    for error in reasons {
        assert_eq!(error.code(), InvalidTransferStateError::CODE);
        assert_eq!(error.name(), InvalidTransferStateError::NAME);
        assert_eq!(
            InvalidTransferStateError::from_code(error.code()),
            Some(error)
        );
        assert_eq!(
            categorize_by_code(error.code()),
            Some(ErrorCategory::Transfer)
        );
    }
}

// ── Error helper functions ────────────────────────────────────────────────────

#[test]
fn token_existence_helpers() {
    assert_eq!(
        require_token_exists(false),
        Err(TokenNotFoundError::TokenNotFound)
    );
    assert!(require_token_exists(true).is_ok());
}

#[test]
fn configuration_and_validation_helpers() {
    assert_eq!(
        invalid_configuration(),
        Err(ConfigurationError::InvalidConfiguration)
    );
    assert!(ensure_valid_configuration(true).is_ok());
    assert_eq!(
        ensure_valid_configuration(false),
        Err(ConfigurationError::InvalidConfiguration)
    );

    assert_eq!(
        invalid_validation(),
        Err(ValidationError::InvalidValidation)
    );
    assert!(ensure_valid_validation(true).is_ok());
    assert_eq!(
        ensure_valid_validation(false),
        Err(ValidationError::InvalidValidation)
    );
}

#[test]
fn owner_helpers() {
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(crate::AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    with_contract(|env| {
        let owner = Address::generate(env);
        crate::token_owner_storage::assign_owner(env, 1, &owner, 1).unwrap();

        assert!(require_owner(env, 1, &owner).is_ok());
        assert_eq!(
            ensure_owner(false),
            Err(UnauthorizedOwnerError::UnauthorizedOwner)
        );
        assert!(ensure_owner(true).is_ok());
        assert!(ensure_token_exists(env, 1).is_ok());
        assert_eq!(
            ensure_token_exists(env, 404),
            Err(TokenNotFoundError::TokenNotFound)
        );
    });
}

// ── Error serialization ───────────────────────────────────────────────────────

#[test]
fn error_serialization_roundtrip_is_lossless() {
    for error in ALL_STANDARDIZED {
        let code = error.code();
        let name = error.name();

        // code ⇄ standardized error must be consistent.
        match code {
            228 => {
                assert_eq!(name, "InvalidConfiguration");
                assert_eq!(ConfigurationError::InvalidConfiguration.code(), code);
            }
            229 => {
                assert_eq!(name, "InvalidValidation");
                assert_eq!(ValidationError::InvalidValidation.code(), code);
            }
            230 => {
                assert_eq!(name, "TokenNotFound");
                assert_eq!(TokenNotFoundError::TokenNotFound.code(), code);
            }
            231 => {
                assert_eq!(name, "UnauthorizedOwner");
                assert_eq!(UnauthorizedOwnerError::UnauthorizedOwner.code(), code);
            }
            _ => {
                let matched = ALL_INITIALIZATION_ERRORS
                    .iter()
                    .find(|e| e.code() == code)
                    .unwrap_or_else(|| panic!("unexpected code {code}"));
                assert_eq!(name, matched.name());
            }
            232 => {
                assert_eq!(name, "InsufficientPayment");
                assert_eq!(InsufficientPaymentError::InsufficientPayment.code(), code);
            }
            233 => {
                assert_eq!(name, "UnsupportedAsset");
                assert_eq!(UnsupportedAssetError::UnsupportedAsset.code(), code);
                assert_eq!(name, "InvalidRecipient");
                assert_eq!(InvalidRecipientError::InvalidRecipient.code(), code);
            }
            233 => {
                assert_eq!(name, "ApprovalAlreadyExists");
                assert_eq!(
                    ApprovalAlreadyExistsError::ApprovalAlreadyExists.code(),
                    code
                );
            }
            234 => {
                assert_eq!(name, "InvalidTransferState");
                assert_eq!(InvalidTransferStateError::Burned.code(), code);
            }
            _ => panic!("unexpected code {code}"),
        }
    }
}

#[test]
fn category_serialization_roundtrip() {
    for category in crate::error_catalog::categories::ALL_CATEGORIES {
        let byte = category.code();
        let decoded = ErrorCategory::from_code(byte);
        assert_eq!(decoded, Some(category));
        assert_eq!(decoded.unwrap().as_str(), category.as_str());
    }
}
