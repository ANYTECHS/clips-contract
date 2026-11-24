//! Creator events under the centralized `events` namespace (issue #962).
//!
//! The canonical payload definition lives in [`crate::types`] and the emitter
//! lives in [`crate::creator_event`]. This module re-exports both under
//! `events::creator`.
//!
//! # Topic
//! - `"creator"` — creator assigned to a token.
//!
//! # Usage
//! ```rust,ignore
//! events::creator::emit_creator_assigned(&env, token_id, &creator, clip_id, ts);
//! ```

// ── Event payload definition ─────────────────────────────────────────────────
pub use crate::types::CreatorAssignedEvent;

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::creator_event::{build_creator_assigned_event, emit_creator_assigned};
