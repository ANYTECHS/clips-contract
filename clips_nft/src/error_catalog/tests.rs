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
    categorize_by_code, ensure_owner, ensure_token_exists, require_owner, require_token_exists,
    ErrorCategory, TokenNotFoundError, UnauthorizedOwnerError,
};

/// Every standardized error produced by the error infrastructure.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum StandardizedError {
    TokenNotFound(TokenNotFoundError),
    UnauthorizedOwner(UnauthorizedOwnerError),
}

impl StandardizedError {
    const fn code(self) -> u32 {
        match self {
            StandardizedError::TokenNotFound(e) => e.code(),
            StandardizedError::UnauthorizedOwner(e) => e.code(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            StandardizedError::TokenNotFound(e) => e.name(),
            StandardizedError::UnauthorizedOwner(e) => e.name(),
        }
    }
}

const ALL_STANDARDIZED: [StandardizedError; 2] = [
    StandardizedError::TokenNotFound(TokenNotFoundError::TokenNotFound),
    StandardizedError::UnauthorizedOwner(UnauthorizedOwnerError::UnauthorizedOwner),
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
        categorize_by_code(TokenNotFoundError::CODE),
        Some(ErrorCategory::Storage)
    );
    assert_eq!(
        categorize_by_code(UnauthorizedOwnerError::CODE),
        Some(ErrorCategory::Ownership)
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

// ── Error names ───────────────────────────────────────────────────────────────

#[test]
fn error_names_match_discriminants() {
    assert_eq!(TokenNotFoundError::NAME, "TokenNotFound");
    assert_eq!(UnauthorizedOwnerError::NAME, "UnauthorizedOwner");
    for error in ALL_STANDARDIZED {
        assert!(!error.name().is_empty());
        let _ = error.code();
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
            230 => {
                assert_eq!(name, "TokenNotFound");
                assert_eq!(TokenNotFoundError::TokenNotFound.code(), code);
            }
            231 => {
                assert_eq!(name, "UnauthorizedOwner");
                assert_eq!(UnauthorizedOwnerError::UnauthorizedOwner.code(), code);
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
