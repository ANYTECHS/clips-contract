//! Operator approval storage.
//!
//! Tracks whether an `operator` is approved to manage all tokens owned by
//! `owner` (analogous to ERC-721 `setApprovalForAll`).
//!
//! # Storage
//! Key: `DataKey::OperatorApproval(owner, operator)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::approval_revoked_event;
use crate::types::DataKey;

/// Approve `operator` to manage all tokens belonging to `owner`.
pub fn save_operator(env: &Env, owner: &Address, operator: &Address) {
    env.storage().persistent().set(
        &DataKey::OperatorApproval(owner.clone(), operator.clone()),
        &true,
    );
    crate::approval_granted_event::emit_approval_granted(env, owner, operator, None);
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

/// Revoke `operator` approval for `owner` and emit an [`ApprovalRevokedEvent`]
/// (issue #931).
///
/// Returns `true` when an approval was actually revoked. A call for an operator
/// that was never approved removes nothing and emits nothing, so a received
/// event always corresponds to a real permission change.
///
/// [`ApprovalRevokedEvent`]: crate::types::ApprovalRevokedEvent
pub fn revoke_operator(env: &Env, owner: &Address, operator: &Address) -> bool {
    if !is_operator(env, owner, operator) {
        return false;
    }
    remove_operator(env, owner, operator);
    approval_revoked_event::emit_operator_approval_revoked(
        env,
        owner,
        operator,
        env.ledger().timestamp(),
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    #[test]
    fn revoke_operator_removes_and_emits() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);
            save_operator(&env, &owner, &operator);

            assert!(revoke_operator(&env, &owner, &operator));
            assert!(!is_operator(&env, &owner, &operator));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn revoke_operator_is_a_noop_without_an_approval() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);
            assert!(!revoke_operator(&env, &owner, &operator));
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
