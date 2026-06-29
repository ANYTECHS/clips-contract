#![cfg(test)]

use crate::storage::errors::StorageError;

#[test]
fn storage_error_variants_have_distinct_codes() {
    assert_ne!(StorageError::StorageNotFound as u32, StorageError::StorageConflict as u32);
    assert_ne!(StorageError::StorageConflict as u32, StorageError::InvalidStorageKey as u32);
    assert_ne!(StorageError::InvalidStorageKey as u32, StorageError::DuplicateRecord as u32);
}

#[test]
fn storage_not_found_code_is_1() {
    assert_eq!(StorageError::StorageNotFound as u32, 1);
}

#[test]
fn storage_conflict_code_is_2() {
    assert_eq!(StorageError::StorageConflict as u32, 2);
}

#[test]
fn invalid_storage_key_code_is_3() {
    assert_eq!(StorageError::InvalidStorageKey as u32, 3);
}

#[test]
fn duplicate_record_code_is_4() {
    assert_eq!(StorageError::DuplicateRecord as u32, 4);
}

#[test]
fn storage_error_is_copy() {
    let e = StorageError::StorageNotFound;
    let _e2 = e; // would fail to compile if not Copy
    let _e3 = e;
}

#[test]
fn storage_error_equality() {
    assert_eq!(StorageError::DuplicateRecord, StorageError::DuplicateRecord);
    assert_ne!(StorageError::DuplicateRecord, StorageError::StorageNotFound);
}

/// Demonstrates using StorageError in a Result context.
#[test]
fn storage_error_in_result() {
    fn lookup(found: bool) -> Result<u32, StorageError> {
        if found {
            Ok(42)
        } else {
            Err(StorageError::StorageNotFound)
        }
    }

    assert!(lookup(true).is_ok());
    assert_eq!(lookup(false), Err(StorageError::StorageNotFound));
}

/// Demonstrates using StorageError for deduplication guard logic.
#[test]
fn duplicate_record_guard() {
    fn insert(exists: bool) -> Result<(), StorageError> {
        if exists {
            Err(StorageError::DuplicateRecord)
        } else {
            Ok(())
        }
    }

    assert!(insert(false).is_ok());
    assert_eq!(insert(true), Err(StorageError::DuplicateRecord));
}
