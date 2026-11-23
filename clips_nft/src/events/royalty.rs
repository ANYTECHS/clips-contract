//! Royalty events under the centralized `events` namespace
//! (issues #695, #928).
//!
//! Groups every royalty-related event — assignment at mint time, configuration
//! updates, payout completion, and permanent freezing — behind a single
//! `events::royalty` namespace.
//!
//! The canonical payload definitions live in [`crate::types`]; the emitters
//! live in the standalone `royalty_*_event` modules. Both are re-exported here
//! so the emission logic is never duplicated.
//!
//! # Topics
//! - `"ryl_asgn"` — royalty assigned to a token.
//! - `"ryl_upd"`  — royalty configuration updated.
//! - `"ryl_paid"` — royalty payment completed.
//! - `"ryl_frz"`  — royalty configuration permanently frozen.
//!
//! # Usage
//! ```rust,ignore
//! events::royalty::emit_royalty_paid(&env, token_id, &payer, &recipient, amount, &asset, &ref, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{RoyaltyAssignedEvent, RoyaltyFrozenEvent, RoyaltyPaidEvent, RoyaltyUpdatedEvent};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::royalty_assigned_event::emit_royalty_assigned;
pub use crate::royalty_frozen_event::emit_royalty_frozen;
pub use crate::royalty_paid_event::emit_royalty_paid;
pub use crate::royalty_updated_event::emit_royalty_updated;
