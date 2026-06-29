//! Operator approval storage.
//!
//! Tracks whether an `operator` is approved to manage all tokens owned by
//! `owner` (analogous to ERC-721 `setApprovalForAll`).
//!
//! # Storage
//! Key: `DataKey::OperatorApproval(owner, operator)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::DataKey;

/// Approve `operator` to manage all tokens belonging to `owner`.
pub fn save_operator(env: &Env, owner: &Address, operator: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::OperatorApproval(owner.clone(), operator.clone()), &true);
}

/// Revoke `operator` approval for `owner`.
pub fn remove_operator(env: &Env, owner: &Address, operator: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::OperatorApproval(owner.clone(), operator.clone()));
}

/// Return `true` if `operator` is approved for all tokens of `owner`.
pub fn is_operator(env: &Env, owner: &Address, operator: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::OperatorApproval(owner.clone(), operator.clone()))
        .unwrap_or(false)
}
