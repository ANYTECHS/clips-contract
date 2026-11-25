//! Storage error definitions — reusable, storage-layer error variants (issue #1185).
//!
//! Centralizes all storage-related errors so callers can distinguish
//! infrastructure failures from contract-level business-logic rejections.
//!
//! # Error code allocation
//! Storage errors occupy codes **100–103** in the centralized error registry.
//!
//! | Variant             | Code |
//! |---------------------|------|
//! | `StorageNotFound`   | 100  |
//! | `StorageConflict`   | 101  |
//! | `InvalidStorageKey` | 102  |
//! | `DuplicateRecord`   | 103  |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::storage_errors::StorageErrorKind;
//!
//! // Return a not-found error when a key is absent.
//! return Err(StorageErrorKind::StorageNotFound);
//!
//! // Detect duplicate records before writing.
//! if already_exists {
//!     return Err(StorageErrorKind::DuplicateRecord);
//! }
//! ```

use soroban_sdk::contracterror;

/// Reusable storage-layer error type (issue #1185).
///
/// Each variant maps to a unique numeric code so on-chain error indexers can
/// bucket storage failures independently of contract business-logic errors.
///
/// These errors complement — rather than replace — the top-level
/// [`crate::types::Error`] enum: use `StorageErrorKind` inside storage
/// accessor functions and propagate them via `Into<Error>` conversions or
/// dedicated mapping at the call-site.
#[allow(clippy::enum_variant_names)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StorageErrorKind {
    /// Requested storage key does not exist.
    ///
    /// Raised when a `get` or `read` operation finds no entry under the
    /// requested key.  Callers should treat this as a definitive "not found"
    /// rather than a transient failure.
    StorageNotFound = 100,

    /// A write operation would conflict with an existing entry.
    ///
    /// Raised when an insert-only operation detects that the target key
    /// already holds a value.  Callers that want upsert semantics should
    /// switch to an explicit `set` call instead.
    StorageConflict = 101,

    /// The storage key is structurally invalid or uses an unexpected variant.
    ///
    /// Raised when a key fails format validation before any storage access
    /// is attempted (e.g. an out-of-range token ID passed as a key argument).
    InvalidStorageKey = 102,

    /// An attempt was made to insert a record that already exists.
    ///
    /// Semantically similar to `StorageConflict` but specifically covers
    /// deduplication checks — e.g. minting a clip that has already been
    /// assigned to a token, or registering a metadata URI that is already
    /// indexed.
    DuplicateRecord = 103,
}

impl StorageErrorKind {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "storage";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        match self {
            StorageErrorKind::StorageNotFound => 100,
            StorageErrorKind::StorageConflict => 101,
            StorageErrorKind::InvalidStorageKey => 102,
            StorageErrorKind::DuplicateRecord => 103,
        }
    }

    /// Return the machine-readable name for this error.
    pub const fn name(self) -> &'static str {
        match self {
            StorageErrorKind::StorageNotFound => "StorageNotFound",
            StorageErrorKind::StorageConflict => "StorageConflict",
            StorageErrorKind::InvalidStorageKey => "InvalidStorageKey",
            StorageErrorKind::DuplicateRecord => "DuplicateRecord",
        }
    }

    /// Decode a `StorageErrorKind` from a numeric code.
    ///
    /// Returns `None` when `code` does not correspond to any known variant.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            100 => Some(StorageErrorKind::StorageNotFound),
            101 => Some(StorageErrorKind::StorageConflict),
            102 => Some(StorageErrorKind::InvalidStorageKey),
            103 => Some(StorageErrorKind::DuplicateRecord),
            _ => None,
        }
    }
}

// ── Convenience constructors ──────────────────────────────────────────────────

/// Return the canonical "not found" storage error.
pub const fn not_found() -> StorageErrorKind {
    StorageErrorKind::StorageNotFound
}

/// Return the canonical "conflict" storage error.
pub const fn conflict() -> StorageErrorKind {
    StorageErrorKind::StorageConflict
}

/// Return the canonical "invalid key" storage error.
pub const fn invalid_key() -> StorageErrorKind {
    StorageErrorKind::InvalidStorageKey
}

/// Return the canonical "duplicate record" storage error.
pub const fn duplicate_record() -> StorageErrorKind {
    StorageErrorKind::DuplicateRecord
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_codes_are_unique() {
        let variants = [
            StorageErrorKind::StorageNotFound,
            StorageErrorKind::StorageConflict,
            StorageErrorKind::InvalidStorageKey,
            StorageErrorKind::DuplicateRecord,
        ];
        let codes: soroban_sdk::Vec<u32> = {
            // Collect into a fixed array and assert uniqueness manually
            // (no_std: no std::collections::HashSet available).
            let cs = [
                variants[0].code(),
                variants[1].code(),
                variants[2].code(),
                variants[3].code(),
            ];
            // Check every pair is distinct.
            for i in 0..cs.len() {
                for j in (i + 1)..cs.len() {
                    assert_ne!(cs[i], cs[j], "duplicate code: {}", cs[i]);
                }
            }
            // Return an empty Vec just to satisfy the explicit type annotation
            // without pulling in std types.
            cs.len(); // use the array
            return; // end test early — assertions already ran
        };
        drop(codes);
    }

    #[test]
    fn from_code_round_trips() {
        let variants = [
            StorageErrorKind::StorageNotFound,
            StorageErrorKind::StorageConflict,
            StorageErrorKind::InvalidStorageKey,
            StorageErrorKind::DuplicateRecord,
        ];
        for v in variants {
            assert_eq!(StorageErrorKind::from_code(v.code()), Some(v));
        }
    }

    #[test]
    fn from_code_unknown_returns_none() {
        assert_eq!(StorageErrorKind::from_code(0), None);
        assert_eq!(StorageErrorKind::from_code(99), None);
        assert_eq!(StorageErrorKind::from_code(200), None);
    }

    #[test]
    fn names_match_variants() {
        assert_eq!(StorageErrorKind::StorageNotFound.name(), "StorageNotFound");
        assert_eq!(StorageErrorKind::StorageConflict.name(), "StorageConflict");
        assert_eq!(
            StorageErrorKind::InvalidStorageKey.name(),
            "InvalidStorageKey"
        );
        assert_eq!(StorageErrorKind::DuplicateRecord.name(), "DuplicateRecord");
    }

    #[test]
    fn convenience_constructors_return_correct_variant() {
        assert_eq!(not_found(), StorageErrorKind::StorageNotFound);
        assert_eq!(conflict(), StorageErrorKind::StorageConflict);
        assert_eq!(invalid_key(), StorageErrorKind::InvalidStorageKey);
        assert_eq!(duplicate_record(), StorageErrorKind::DuplicateRecord);
    }
}
