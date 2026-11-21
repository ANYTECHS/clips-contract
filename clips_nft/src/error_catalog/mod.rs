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
//!
//! # Categories
//!
//! The [`categories`] module defines the full set of [`ErrorCategory`]
//! constants used to classify contract failures:
//!
//! | Category        | Description                                              |
//! |-----------------|----------------------------------------------------------|
//! | `Validation`    | Input or state failed validation checks.                 |
//! | `Authorization` | Caller lacks the required permission.                    |
//! | `Ownership`     | Caller does not own the NFT being operated on.           |
//! | `Payment`       | Payment or funds handling failed.                        |
//! | `Marketplace`   | Marketplace listing or sale operation failed.            |
//! | `Royalty`       | Royalty configuration or distribution failed.            |
//! | `Storage`       | Storage lookup or persistence failed.                    |
//! | `Security`      | Security invariant or guard was violated.                |
//! | `Configuration` | Contract configuration is missing or invalid.            |

pub mod categories;
pub mod token_not_found;
pub mod unauthorized_owner;

pub use categories::{categorize_by_code, ErrorCategory, ALL_CATEGORIES};
pub use token_not_found::{ensure_token_exists, require_token_exists, TokenNotFoundError};
pub use unauthorized_owner::{ensure_owner, is_owner, require_owner, UnauthorizedOwnerError};

#[cfg(test)]
pub mod tests;