//! Burn events under the centralized `events` namespace (issue #916).
//!
//! The canonical payload definition lives in [`crate::types`] and the emitter
//! lives in [`crate::burn_event`]. This module re-exports both under
//! `events::burn`, keeping the topic label (`"nft_burn"`) defined in exactly
//! one place.
//!
//! # Usage
//! ```rust,ignore
//! events::burn::emit_nft_burned(&env, token_id, &previous_owner, &caller, ts);
//! ```

// ── Event payload definition ─────────────────────────────────────────────────
pub use crate::types::NFTBurnedEvent;

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::burn_event::{build_nft_burned_event, emit_nft_burned};
