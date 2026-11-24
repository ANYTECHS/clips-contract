//! Tests for the centralized error infrastructure (issue #1381).
//!
//! These tests verify that the error registry, categories, names, helper
//! functions, and serialization behave consistently across the contract.

use crate::errors::{
    config_error, validation_error, Error, ErrorCategory, ErrorCode, ErrorRegistry,
};
use soroban_sdk::{testutils::Address as _, Address, Env, String, Symbol};

// -------------------------------------------------------------------------
// Registry: unique codes
// -------------------------------------------------------------------------

#[test]
fn test_all_error_codes_are_unique() {
    let env = Env::default();
    let registry = ErrorRegistry::all(&env);

    let mut seen: Vec<u32> = Vec::new();
    for entry in registry.iter() {
        let code = entry.code as u32;
        assert!(
            !seen.contains(&code),
            "duplicate error code detected: {}",
            code
        );
        seen.push(code);
    }
    assert!(!seen.is_empty(), "error registry must not be empty");
}

// -------------------------------------------------------------------------
// Registry: names and descriptions
// -------------------------------------------------------------------------

#[test]
fn test_all_entries_have_non_empty_name_and_description() {
    let env = Env::default();
    let registry = ErrorRegistry::all(&env);

    for entry in registry.iter() {
        assert!(
            !entry.name.is_empty(),
            "error code {} has an empty name",
            entry.code as u32
        );
        assert!(
            !entry.description.is_empty(),
            "error code {} has an empty description",
            entry.code as u32
        );
    }
}

// -------------------------------------------------------------------------
// Registry: categories
// -------------------------------------------------------------------------

#[test]
fn test_every_code_maps_to_a_category() {
    let env = Env::default();
    let registry = ErrorRegistry::all(&env);

    for entry in registry.iter() {
        let category = ErrorRegistry::category_of(&env, entry.code);
        assert_eq!(
            category, entry.category,
            "category mismatch for error code {}",
            entry.code as u32
        );
        // Every category must be one of the known variants.
        match category {
            ErrorCategory::Configuration
            | ErrorCategory::Validation
            | ErrorCategory::NotFound
            | ErrorCategory::Authorization
            | ErrorCategory::State
            | ErrorCategory::Unknown => {}
        }
    }
}

#[test]
fn test_category_lookup_for_known_codes() {
    let env = Env::default();

    assert_eq!(
        ErrorRegistry::category_of(&env, ErrorCode::InvalidConfiguration),
        ErrorCategory::Configuration
    );
    assert_eq!(
        ErrorRegistry::category_of(&env, ErrorCode::InvalidInput),
        ErrorCategory::Validation
    );
    assert_eq!(
        ErrorRegistry::category_of(&env, ErrorCode::TokenNotFound),
        ErrorCategory::NotFound
    );
}

// -------------------------------------------------------------------------
// Helper functions
// -------------------------------------------------------------------------

#[test]
fn test_config_error_helper_returns_standardized_error() {
    let env = Env::default();
    let err = config_error(&env, ErrorCode::InvalidConfiguration);

    assert_eq!(err.code, ErrorCode::InvalidConfiguration);
    assert_eq!(err.category, ErrorCategory::Configuration);
    assert!(!err.message.is_empty());
}

#[test]
fn test_validation_error_helper_returns_standardized_error() {
    let env = Env::default();
    let err = validation_error(&env, ErrorCode::InvalidInput);

    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.category, ErrorCategory::Validation);
    assert!(!err.message.is_empty());
}

#[test]
fn test_helpers_produce_consistent_categories() {
    let env = Env::default();

    let cfg = config_error(&env, ErrorCode::InvalidConfiguration);
    assert_eq!(
        cfg.category,
        ErrorRegistry::category_of(&env, ErrorCode::InvalidConfiguration)
    );

    let val = validation_error(&env, ErrorCode::InvalidInput);
    assert_eq!(
        val.category,
        ErrorRegistry::category_of(&env, ErrorCode::InvalidInput)
    );
}

// -------------------------------------------------------------------------
// Serialization round-trips
// -------------------------------------------------------------------------

#[test]
fn test_configuration_error_serialization_round_trip() {
    let env = Env::default();
    let original = config_error(&env, ErrorCode::InvalidConfiguration);

    let code: u32 = original.code.into();
    let restored = Error::from_code(&env, code);

    assert_eq!(restored.code, original.code);
    assert_eq!(restored.category, original.category);
    assert_eq!(restored.message, original.message);
}

#[test]
fn test_validation_error_serialization_round_trip() {
    let env = Env::default();
    let original = validation_error(&env, ErrorCode::InvalidInput);

    let code: u32 = original.code.into();
    let restored = Error::from_code(&env, code);

    assert_eq!(restored.code, original.code);
    assert_eq!(restored.category, original.category);
    assert_eq!(restored.message, original.message);
}

#[test]
fn test_token_not_found_error_serialization_round_trip() {
    let env = Env::default();
    let original = Error::new(&env, ErrorCode::TokenNotFound);

    let code: u32 = original.code.into();
    let restored = Error::from_code(&env, code);

    assert_eq!(restored.code, original.code);
    assert_eq!(restored.category, ErrorCategory::NotFound);
    assert_eq!(restored.message, original.message);
}

#[test]
fn test_all_registry_codes_round_trip() {
    let env = Env::default();
    let registry = ErrorRegistry::all(&env);

    for entry in registry.iter() {
        let code: u32 = entry.code.into();
        let restored = Error::from_code(&env, code);
        assert_eq!(
            restored.code, entry.code,
            "round-trip failed for error code {}",
            code
        );
        assert_eq!(
            restored.category, entry.category,
            "round-trip category mismatch for error code {}",
            code
        );
    }
}

// -------------------------------------------------------------------------
// Misc consistency checks
// -------------------------------------------------------------------------

#[test]
fn test_error_name_lookup_matches_registry() {
    let env = Env::default();
    let registry = ErrorRegistry::all(&env);

    for entry in registry.iter() {
        let name = ErrorRegistry::name_of(&env, entry.code);
        assert_eq!(
            name, entry.name,
            "name mismatch for error code {}",
            entry.code as u32
        );
    }
}

#[test]
fn test_error_symbols_are_valid() {
    let env = Env::default();
    let registry = ErrorRegistry::all(&env);

    for entry in registry.iter() {
        let symbol: Symbol = entry.name.clone();
        // Symbols must be non-empty and within the allowed length.
        let len = symbol.len();
        assert!(len > 0, "empty symbol for error code {}", entry.code as u32);
        assert!(
            len <= 32,
            "symbol too long for error code {}",
            entry.code as u32
        );
    }
}

#[test]
fn test_unknown_code_maps_to_unknown_category() {
    let env = Env::default();
    // A code that is not present in the registry.
    let unknown_code: u32 = 0xFFFF_FFFF;
    let category = ErrorRegistry::category_of_u32(&env, unknown_code);
    assert_eq!(category, ErrorCategory::Unknown);
}

#[test]
fn test_error_message_is_string() {
    let env = Env::default();
    let err = config_error(&env, ErrorCode::InvalidConfiguration);
    let _msg: String = err.message.clone();
    assert!(!_msg.is_empty());
}

#[test]
fn test_address_unused_smoke() {
    // Ensures the test module compiles with the Address import used elsewhere.
    let env = Env::default();
    let _addr = Address::generate(&env);
}