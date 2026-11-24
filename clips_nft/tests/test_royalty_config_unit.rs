#![cfg(test)]

use clips_nft::{RoyaltyValidationError, RoyaltyErrorCode};

#[test]
fn royalty_above_maximum_returns_error() {
    let err = RoyaltyValidationError::RoyaltyAboveMaximum {
        provided: 10_001,
        maximum: 10_000,
    };
    assert_eq!(err.code(), RoyaltyErrorCode::RoyaltyAboveMaximum);
    assert_eq!(err.code() as u32, 1);
}

#[test]
fn invalid_recipient_returns_error() {
    let err = RoyaltyValidationError::InvalidRecipient;
    assert_eq!(err.code(), RoyaltyErrorCode::InvalidRecipient);
    assert_eq!(err.code() as u32, 2);
}

#[test]
fn invalid_bps_returns_error() {
    let err = RoyaltyValidationError::InvalidBps { provided: 20_000 };
    assert_eq!(err.code(), RoyaltyErrorCode::InvalidBps);
    assert_eq!(err.code() as u32, 3);
}

#[test]
fn invalid_royalty_state_returns_error() {
    let err = RoyaltyValidationError::InvalidRoyaltyState;
    assert_eq!(err.code(), RoyaltyErrorCode::InvalidRoyaltyState);
    assert_eq!(err.code() as u32, 4);
}

#[test]
fn unauthorized_update_returns_error() {
    let err = RoyaltyValidationError::UnauthorizedUpdate;
    assert_eq!(err.code(), RoyaltyErrorCode::UnauthorizedUpdate);
    assert_eq!(err.code() as u32, 5);
}

#[test]
fn error_codes_are_unique() {
    let codes = [
        RoyaltyErrorCode::RoyaltyAboveMaximum as u32,
        RoyaltyErrorCode::InvalidRecipient as u32,
        RoyaltyErrorCode::InvalidBps as u32,
        RoyaltyErrorCode::InvalidRoyaltyState as u32,
        RoyaltyErrorCode::UnauthorizedUpdate as u32,
    ];
    let mut sorted = codes;
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), codes.len(), "error codes must be unique");
}