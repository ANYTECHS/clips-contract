//! Reusable transfer and minting guard errors for the ClipCash contract.
//!
//! Companion to the centralized error infrastructure and the standardized
//! error catalog. This module provides reusable guard errors triggered when
//! a transfer or minting operation would violate a core restriction:
//!
//! * [`InvalidRecipientError`] — the recipient address is not a usable
//!   recipient (`#989`).
//! * [`SelfTransferNotAllowedError`] — the sender is transferring the token
//!   to itself (`#990`).
//! * [`FrozenTokenError`] — the token is frozen and cannot be moved (`#991`).
//! * [`TokenAlreadyExistsError`] — the token ID is already taken (`#992`).
//!
//! # Error code allocation
//!
//! These codes stay unique across the whole contract and slot into the
//! error-classification buckets (validation, security) defined by the errors
//! issue series:
//!
//! | Module     | Code | Error                   | Bucket     |
//! |------------|------|-------------------------|------------|
//! | `transfer` | 240  | `InvalidRecipient`      | validation|
//! | `transfer` | 241  | `SelfTransferNotAllowed`| validation|
//! | `transfer` | 242  | `FrozenToken`           | security  |
//! | `minting`  | 250  | `TokenAlreadyExists`    | security  |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::reusable_errors::{frozen_token, invalid_recipient};
//!
//! invalid_recipient::ensure_valid_recipient(env, &recipient)?;
//! frozen_token::require_not_frozen(env, token_id)?;
//! ```

pub mod already_exists;
pub mod frozen_token;
pub mod invalid_recipient;
pub mod self_transfer;

pub use already_exists::{
    ensure_token_does_not_exist, ensure_unique_token, TokenAlreadyExistsError,
};
pub use frozen_token::{ensure_not_frozen, is_token_frozen, require_not_frozen, FrozenTokenError};
pub use invalid_recipient::{
    ensure_recipient, ensure_valid_recipient, is_valid_recipient, InvalidRecipientError,
};
pub use self_transfer::{ensure_no_self_transfer, is_self_transfer, SelfTransferNotAllowedError};
