//! Batch-mint events under the centralized `events` namespace (issue #697).
//!
//! The canonical payload definition lives in [`crate::types`] and the emitter
//! lives in [`crate::batch_mint_event`]. This module re-exports both under
//! `events::batch_mint`.
//!
//! # Topic
//! - `"btch_done"` — a batch mint completed successfully.
//!
//! # Usage
//! ```rust,ignore
//! events::batch_mint::emit_batch_mint_completed(&env, batch_id, minted_count, &recipient, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{BatchId, BatchMintCompletedEvent};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::batch_mint_event::emit_batch_mint_completed;
