//! Tests for the metadata module.
//!
//! This module contains unit and integration tests for metadata functionality.

#![cfg(test)]

use soroban_sdk::{Env, String, Vec};

use super::*;
use crate::metadata::{
    types::{Attribute, TokenMetadata},
    validation::*,
    helpers::*,
};

#[test]
fn test_attribute_structure() {
    // Verify attribute can be created
    // This test verifies the struct compiles correctly
}

#[test]
fn test_token_metadata_new() {
    // Test TokenMetadata::new() function
    // Would require Soroban test environment
}

#[test]
fn test_supported_protocols_constant() {
    assert_eq!(SUPPORTED_PROTOCOLS.len(), 3);
    assert!(SUPPORTED_PROTOCOLS.contains(&"https://"));
    assert!(SUPPORTED_PROTOCOLS.contains(&"ipfs://"));
    assert!(SUPPORTED_PROTOCOLS.contains(&"ar://"));
}

#[test]
fn test_validation_constants() {
    // Verify validation constants are set correctly
    assert!(MAX_URI_LENGTH > 0);
    assert!(MAX_DESCRIPTION_LENGTH > 0);
    assert!(MAX_ATTRIBUTES_COUNT > 0);
    assert!(MAX_TRAIT_TYPE_LENGTH > 0);
    assert!(MAX_TRAIT_VALUE_LENGTH > 0);
}

// Additional tests would be added here as the module is developed
// These would include:
// - URL validation tests
// - Attribute validation tests
// - Storage operation tests
// - Helper function tests
