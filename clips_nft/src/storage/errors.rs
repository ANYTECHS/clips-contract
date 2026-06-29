//! Storage-specific error definitions — resolves issue #534.
//!
//! Provides a dedicated error type for storage layer operations.
//! Separating storage errors from contract-level errors allows callers to
//! distinguish infrastructure failures from business-logic rejections.

use soroban_sdk::contracterror;

/// Errors that may arise from storage read / write operations.
///
/// These are distinct from the top-level [`crate::types::Error`] variants, which
/// represent contract-level business-logic failures.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StorageError {
    /// Requested key does not exist in storage.
    StorageNotFound = 1,
    /// A write would conflict with an existing entry (e.g. duplicate key).
    StorageConflict = 2,
    /// The storage key is malformed or uses an unexpected variant.
    InvalidStorageKey = 3,
    /// An attempt was made to insert a record that already exists.
    DuplicateRecord = 4,
}
