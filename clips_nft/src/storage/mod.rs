//! Storage module — resolves issue #490.
//!
//! Organises all persistent and instance storage helpers in one place.
//!
//! # Sub-modules
//! - [`keys`] — compact enum of all storage keys
//! - [`config`] — [`Config`] getter / setter / validator
//! - [`errors`] — storage-layer error definitions (issue #534)

pub mod config;
pub mod errors;
pub mod keys;

// Re-export the most-used helpers so callers can write `storage::get_config`.
pub use config::{get_config, set_config, validate_config};
pub use errors::StorageError;
pub use keys::StorageKey;
