//! Freeze / unfreeze events under the centralized `events` namespace
//! (issues #917, #918).
//!
//! The canonical payload definitions live in [`crate::types`] and the emitters
//! live in [`crate::nft_frozen_event`] / [`crate::nft_unfrozen_event`]. This
//! module groups both halves of the freeze lifecycle under a single
//! `events::freeze` namespace.
//!
//! # Topics
//! - `"nft_frz"`  — token frozen (transfer disabled).
//! - `"nft_unfrz"` — token unfrozen (transfer re-enabled).
//!
//! # Usage
//! ```rust,ignore
//! events::freeze::emit_nft_frozen(&env, token_id, &caller, reason.as_ref(), ts);
//! events::freeze::emit_nft_unfrozen(&env, token_id, &caller, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{NFTFrozenEvent, NFTUnfrozenEvent};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::nft_frozen_event::{build_nft_frozen_event, emit_nft_frozen};
pub use crate::nft_unfrozen_event::{build_nft_unfrozen_event, emit_nft_unfrozen};
