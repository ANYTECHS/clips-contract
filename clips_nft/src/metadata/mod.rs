//! # Metadata Module
//!
//! This module is responsible for managing all NFT metadata structures,
//! helpers, and validation logic for the ClipsNFT contract.
//!
//! ## Overview
//!
//! The metadata module provides:
//! - **Types**: Core metadata structures (`Attribute`, `ClipMetadata`, `TokenMetadata`)
//! - **Validation**: URL protocol validation, metadata field validation
//! - **Storage**: Metadata persistence and retrieval operations
//! - **Helpers**: Utility functions for metadata manipulation and JSON generation
//!
//! ## Structure
//!
//! - `types.rs` - Core metadata type definitions
//! - `validation.rs` - Metadata validation logic
//! - `storage.rs` - Metadata storage operations
//! - `helpers.rs` - Utility functions for metadata operations
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::metadata::{ClipMetadata, validate_metadata_uri, save_metadata};
//!
//! // Validate a metadata URL
//! validate_metadata_uri(&env, &uri)?;
//!
//! // Save metadata for a token
//! save_metadata(&env, token_id, &uri);
//! ```

mod constants;
mod errors;
mod helpers;
mod metadata_builder;
mod storage;
#[cfg(test)]
mod tests;
pub mod types;
pub mod validation;

// Re-export public types
pub use types::{Attribute, ClipMetadata, MetadataImage, TokenMetadata};

// Re-export metadata errors
pub use errors::MetadataError;

// Re-export constants
pub use constants::*;

// Re-export validation functions
pub use validation::{
    validate_animation_url, validate_attributes, validate_description, validate_external_url,
    validate_image_url, validate_metadata_uri, validate_url, SUPPORTED_PROTOCOLS,
};

// Re-export storage functions
pub use storage::{get_metadata, metadata_exists, remove_metadata, save_metadata, update_metadata};

// Re-export helper functions
pub use helpers::{
    build_metadata_json, clear_optional_field, filter_empty_attributes, has_duplicate_traits,
    is_empty_string, normalize_url,
};

// Re-export builder utilities
pub use metadata_builder::{ClipMetadataBuilder, TokenMetadataBuilder};

