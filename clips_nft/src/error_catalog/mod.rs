//! Standardized error catalog for the ClipCash contract.
//!
//! Companion to the centralized error infrastructure. This module provides:
//!
//! * [`categories`] — error category constants that make it easier for
//!   developers and indexers to classify contract failures (`#985`).
//! * [`token_not_found`] — the standardized [`TokenNotFoundError`] reused
//!   across NFT modules (`#987`).
//! * [`unauthorized_owner`] — the standardized [`UnauthorizedOwnerError`]
//!   used during transfers and marketplace operations (`#988`).
//! * [`batch_transfer`] — the standardized [`BatchTransferError`] used for
//!   invalid batch transfer operations (`#1394`).
//! * [`listing_not_found`] — the standardized [`ListingNotFoundError`] used
//!   across marketplace operations (`#1396`).
//! * [`listing_already_exists`] — the standardized [`ListingAlreadyExistsError`]
//!   used when creating a duplicate active listing (`#1397`).
//! * [`listing_expired`] — the standardized [`ListingExpiredError`] used when
//!   interacting with an expired marketplace listing (`#1398`).
//! * [`tests`] — tests verifying the centralized error infrastructure works
//!   consistently (`#986`).
//!
//! # Error codes
//!
//! | Code | Error                    | Category  |
//! |------|--------------------------|-----------|
//! | 230  | `TokenNotFound`          | storage   |
//! | 231  | `UnauthorizedOwner`      | ownership |
//! | 240  | `BatchTransfer`          | validation |
//! | 250  | `ListingNotFound`        | marketplace |
//! | 251  | `ListingAlreadyExists`   | marketplace |
//! | 252  | `ListingExpired`         | marketplace |
//!
//! Codes are unique and match the centralized error registry.

pub mod batch_transfer;
pub mod categories;
pub mod listing_already_exists;
pub mod listing_expired;
pub mod listing_not_found;
pub mod token_not_found;
pub mod unauthorized_owner;

pub use batch_transfer::{
    ensure_batch_valid, require_batch_valid, BatchTransferError, BatchTransferErrorCode,
};
pub use categories::{categorize_by_code, ErrorCategory};
pub use listing_already_exists::{
    ensure_listing_not_exists, require_listing_not_exists, ListingAlreadyExistsError,
};
pub use listing_expired::{ensure_listing_not_expired, require_listing_not_expired, ListingExpiredError};
pub use listing_not_found::{
    ensure_listing_exists, require_listing_exists, ListingNotFoundError,
};
pub use token_not_found::{ensure_token_exists, require_token_exists, TokenNotFoundError};
pub use unauthorized_owner::{ensure_owner, is_owner, require_owner, UnauthorizedOwnerError};

#[cfg(test)]
pub mod tests;