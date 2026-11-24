//! Configuration events under the centralized `events` namespace (issue #932).
//!
//! The canonical payload definitions live in [`crate::types`] and the emitters
//! (including the "only emit when the value actually changed" helpers) live in
//! [`crate::config_updated_event`]. This module re-exports both under
//! `events::config`.
//!
//! # Topic
//! - `"cfg_updt"` — a contract-wide configuration value changed.
//!
//! # Usage
//! ```rust,ignore
//! events::config::emit_config_updated(&env, field, old_value, new_value, &admin, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{ConfigField, ConfigUpdatedEvent, ConfigValue};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::config_updated_event::{
    emit_address_change, emit_bool_change, emit_config_updated, emit_numeric_change,
    emit_string_change,
};
