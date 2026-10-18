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
//! * [`tests`] — tests verifying the centralized error infrastructure works
//!   consistently (`#986`).
//!
//! # Error codes
//!
//! | Code | Error                | Category  |
//! |------|----------------------|-----------|
//! | 230  | `TokenNotFound`      | storage   |
//! | 231  | `UnauthorizedOwner`  | ownership |
//!
//! Codes are unique and match the centralized error registry.

pub mod categories;
pub mod token_not_found;
pub mod unauthorized_owner;

pub use categories::{categorize_by_code, ErrorCategory};
pub use token_not_found::{ensure_token_exists, require_token_exists, TokenNotFoundError};
pub use unauthorized_owner::{ensure_owner, is_owner, require_owner, UnauthorizedOwnerError};

#[cfg(test)]
pub mod tests;
