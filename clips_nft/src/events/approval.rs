//! Approval events under the centralized `events` namespace (issue #931).
//!
//! Groups approval and operator-permission events. The canonical payload
//! definitions live in [`crate::types`]; the emitters live in
//! [`crate::approval_granted_event`] and [`crate::approval_revoked_event`].
//! Both are re-exported here so the emission logic is never duplicated.
//!
//! # Topics
//! - `"approval"`  — token or operator approval granted.
//! - `"aprv_rvkd"` — approval or operator permission revoked.
//!
//! # Usage
//! ```rust,ignore
//! events::approval::emit_approval_granted(&env, &owner, &operator, Some(token_id));
//! events::approval::emit_token_approval_revoked(&env, &owner, &approved, token_id, ts);
//! ```

// ── Event payload definitions ────────────────────────────────────────────────
pub use crate::types::{ApprovalGrantedEvent, ApprovalRevokedEvent, ApprovalScope};

// ── Emission helpers ─────────────────────────────────────────────────────────
pub use crate::approval_granted_event::emit_approval_granted;
pub use crate::approval_revoked_event::{
    emit_approval_revoked, emit_operator_approval_revoked, emit_token_approval_revoked,
};
