//! Centralized error infrastructure for the ClipCash contract.
//!
//! This module is the single source of truth for the standardized, reusable
//! error space described by the "[Errors]" issue series. It provides:
//!
//! * [`registry`] — a centralized registry of unique error codes grouped by
//!   module (`#981`).
//! * [`initialization`] — errors related to contract initialization (`#982`).
//! * [`configuration`] — errors for invalid contract configuration
//!   operations (`#983`).
//! * [`validation`] — reusable helpers that return standardized validation
//!   errors (`#984`).
//!
//! # Error code allocation
//!
//! Each module owns a contiguous, non-overlapping block so codes are unique
//! across the whole contract and indexers can bucket failures reliably:
//!
//! | Module          | Codes    |
//! |-----------------|----------|
//! | `initialization`| 200–204  |
//! | `configuration` | 210–214  |
//! | `validation`    | 220–225  |
//!
//! The [`registry`] is kept in lock-step with the enums defined here and is
//! guarded by tests that fail on duplicate codes or names.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::error_infrastructure::{InitializationError, ValidationError};
//!
//! if !initialized {
//!     return Err(InitializationError::ContractNotInitialized);
//! }
//! validation::ensure_valid_address(true)?;
//! ```

pub mod configuration;
pub mod initialization;
pub mod registry;
pub mod validation;

pub use configuration::ConfigurationError;
pub use initialization::InitializationError;
pub use registry::{codes_for_module, error_code, name_for, ErrorCode};
pub use validation::ValidationError;
