//! Contract pause / unpause events under the centralized `events` namespace
//! (issues #933, #934).
//!
//! The canonical payload definitions live in [`crate::types`] and the emitters
//! live in [`crate::pause_event`]. This module re-exports both under
//! `events::pause`.
//!
//! # Topic
//! - `"cfg_pause"` — contract paused or resumed.
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
