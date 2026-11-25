//! Contract pause / unpause events under the centralized `events` namespace
//! (issues #933, #934, #1372).
//!
//! The canonical payload definitions live in [`crate::types`] and the emitters
//! live in [`crate::pause_event`]. This module re-exports both under
//! `events::pause`.
//!
//! # Topic
//! - `"cfg_pause"` — contract paused or resumed.
//!
//! # Events
//! - [`ContractPausedEvent`] — emitted when the contract is paused.
//! - [`ContractUnpausedEvent`] — emitted when the contract is resumed after
//!   being paused (issue #1372). Carries the administrator who performed the
//!   unpause and the timestamp of the operation.
//!
//! # Usage
//! ```rust,ignore
//! events::pause::emit_contract_paused(&env, &admin, reason, ts);
//! events::pause::emit_contract_unpaused(&env, &admin, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{ContractPausedEvent, ContractUnpausedEvent};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::pause_event::{emit_contract_paused, emit_contract_unpaused};