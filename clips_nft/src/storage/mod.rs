//! Contract storage module — issue #1151.
//!
//! Organises all persistent and instance storage helpers behind one module tree
//! so entry points never touch raw storage keys directly.
//!
//! # Sub-modules
//!
//! | Module            | Issue  | Responsibility                                    |
//! |-------------------|--------|---------------------------------------------------|
//! | [`keys`]          | #1151  | Compact enum of every storage key used on-chain   |
//! | [`config`]        | #1153  | [`Config`] getter / setter / validator            |
//! | [`owner`]         | #1154  | Contract owner address (save / read / update)     |
//! | [`administrator`] | #1155  | Multi-admin set (add / remove / check)            |
//! | [`errors`]        | #1151  | Storage-layer error definitions                   |
//!
//! # Re-exports
//!
//! The most commonly used helpers are re-exported from the crate root of this
//! module so callers can write `storage::get_config(env)` without importing
//! the nested path.

pub mod administrator;
pub mod config;
pub mod errors;
pub mod keys;
pub mod owner;

// ── Convenience re-exports ────────────────────────────────────────────────────

// Config helpers (issue #1153)
pub use config::{get_config, set_config, validate_config};

// Owner helpers (issue #1154)
pub use owner::{get_owner, save_owner, update_owner};

// Administrator helpers (issue #1155)
pub use administrator::{add_admin, is_admin, remove_admin};

// Storage-layer error type
pub use errors::StorageError;

// Storage key enum
pub use keys::StorageKey;
