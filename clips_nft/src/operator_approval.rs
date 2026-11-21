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
    let _ = save_operator_checked(env, owner, operator);
}

pub fn save_operator_checked(
    env: &Env,
    owner: &Address,
    operator: &Address,
) -> Result<(), crate::types::Error> {
    if is_operator(env, owner, operator) {
        return Err(crate::types::Error::ApprovalAlreadyExists);
    }
    env.storage().persistent().set(
        &DataKey::OperatorApproval(owner.clone(), operator.clone()),
        &true,
    );
    crate::approval_granted_event::emit_approval_granted(env, owner, operator, None);
    Ok(())
}

/// Revoke `operator` approval for `owner`.
pub fn remove_operator(env: &Env, owner: &Address, operator: &Address) {
    if is_operator(env, owner, operator) {
        env.storage()
            .persistent()
            .remove(&DataKey::OperatorApproval(owner.clone(), operator.clone()));
        approval_revoked_event::emit_operator_approval_revoked(
            env,
            owner,
            operator,
            env.ledger().timestamp(),
        );
    }
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
    true
}

pub fn revoke_operator_checked(
    env: &Env,
    owner: &Address,
    operator: &Address,
) -> Result<(), crate::types::Error> {
    if !is_operator(env, owner, operator) {
        return Err(crate::types::Error::ApprovalNotFound);
    }
    remove_operator(env, owner, operator);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger},
        xdr::ContractEventBody,
        Address, Env, Symbol, TryFromVal,
    };
    use crate::{
        event_topics::TOPIC_APPROVAL_REVOKED,
        types::{ApprovalRevokedEvent, ApprovalScope},
    };

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    #[test]
    fn operator_can_be_saved_checked_and_removed() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);

            assert!(!is_operator(&env, &owner, &operator));

            save_operator(&env, &owner, &operator);
            assert!(is_operator(&env, &owner, &operator));

            remove_operator(&env, &owner, &operator);
            assert!(!is_operator(&env, &owner, &operator));
        });
    }

    #[test]
    fn revoke_operator_removes_and_emits() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);
            let timestamp = 1_700_000_000;
            env.ledger().set_timestamp(timestamp);
            save_operator(&env, &owner, &operator);

            assert!(revoke_operator(&env, &owner, &operator));
            assert!(!is_operator(&env, &owner, &operator));
            let events = env.events().all();
            assert_eq!(events.events().len(), 2);
            let ContractEventBody::V0(event) = &events.events()[1].body else {
                panic!("expected a V0 contract event");
            };
            assert_eq!(event.topics.len(), 1);
            assert_eq!(
                Symbol::try_from_val(&env, &event.topics[0]),
                Ok(TOPIC_APPROVAL_REVOKED)
            );
            assert_eq!(
                ApprovalRevokedEvent::try_from_val(&env, &event.data).unwrap(),
                ApprovalRevokedEvent {
                    owner,
                    approved: operator,
                    scope: ApprovalScope::AllTokens,
                    timestamp,
                }
            );
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
