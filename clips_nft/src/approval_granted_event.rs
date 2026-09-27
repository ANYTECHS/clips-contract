//! Approval Granted event — emitted when token or operator approval is granted.
//!
//! Emits an event to track when approvals are given on tokens or globally.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{ApprovalGrantedEvent, TokenId};

/// Emit the `"approval"` event when an approval is granted.
///
/// If `token_id` is `Some`, it's a single-token approval.
/// If `token_id` is `None`, it's an operator approval for all tokens of `owner`.
pub fn emit_approval_granted(
    env: &Env,
    owner: &Address,
    operator: &Address,
    token_id: Option<TokenId>,
) {
    env.events().publish(
        (symbol_short!("approval"), owner.clone(), operator.clone()),
        ApprovalGrantedEvent {
            owner: owner.clone(),
            operator: operator.clone(),
            token_id,
            timestamp: env.ledger().timestamp(),
        },
    );
}
