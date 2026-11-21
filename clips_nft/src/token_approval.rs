//! Token approval storage.
//!
//! Tracks which address is approved to transfer a specific token on behalf of
//! its owner (single-token approval, analogous to ERC-721 `approve`).
//!
//! # Storage
//! Key: `DataKey::Approval(token_id)` (persistent storage)

use crate::approval_revoked_event;
use crate::types::DataKey;
use soroban_sdk::{Address, Env};

/// Persist an approval: `approved` may transfer `token_id`.
pub fn save_approval(env: &Env, token_id: u32, approved: &Address) {
    let _ = save_approval_checked(env, token_id, approved);
}

pub fn save_approval_checked(
    env: &Env,
    token_id: u32,
    approved: &Address,
) -> Result<(), crate::types::Error> {
    if get_approval(env, token_id).is_some() {
        return Err(crate::types::Error::ApprovalAlreadyExists);
    }
    env.storage()
        .persistent()
        .set(&DataKey::Approval(token_id), approved);
    if let Ok(owner) = crate::token_owner_storage::get_owner(env, token_id) {
        crate::approval_granted_event::emit_approval_granted(env, &owner, approved, Some(token_id));
    }
    Ok(())
}

/// Remove any existing approval for `token_id`.
pub fn remove_approval(env: &Env, token_id: u32) {
    env.storage()
        .persistent()
        .remove(&DataKey::Approval(token_id));
}

/// Return the currently approved address for `token_id`, if any.
pub fn get_approval(env: &Env, token_id: u32) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Approval(token_id))
}

/// Revoke the approval for `token_id` and emit an [`ApprovalRevokedEvent`]
/// (issue #931).
///
/// Returns the address that lost the approval, or `None` when no approval was
/// set — in which case nothing is removed and no event is emitted, so a
/// received event always corresponds to a real permission change.
///
/// [`ApprovalRevokedEvent`]: crate::types::ApprovalRevokedEvent
pub fn revoke_approval(env: &Env, owner: &Address, token_id: u32) -> Option<Address> {
    let approved = get_approval(env, token_id)?;
    remove_approval(env, token_id);
    approval_revoked_event::emit_token_approval_revoked(
        env,
        owner,
        &approved,
        token_id,
        env.ledger().timestamp(),
    );
    Some(approved)
}

pub fn revoke_approval_checked(
    env: &Env,
    owner: &Address,
    token_id: u32,
) -> Result<Address, crate::types::Error> {
    revoke_approval(env, owner, token_id).ok_or(crate::types::Error::ApprovalNotFound)
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
    fn approval_can_be_saved_retrieved_and_removed() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let approved = Address::generate(&env);
            assert_eq!(get_approval(&env, 1), None);

            save_approval(&env, 1, &approved);
            assert_eq!(get_approval(&env, 1), Some(approved));

            remove_approval(&env, 1);
            assert_eq!(get_approval(&env, 1), None);
        });
    }

    #[test]
    fn revoke_approval_removes_and_emits() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let approved = Address::generate(&env);
            let timestamp = 1_700_000_000;
            env.ledger().set_timestamp(timestamp);
            save_approval(&env, 1, &approved);

            assert_eq!(revoke_approval(&env, &owner, 1), Some(approved.clone()));
            assert_eq!(get_approval(&env, 1), None);
            let events = env.events().all();
            assert_eq!(events.events().len(), 1);
            let ContractEventBody::V0(event) = &events.events()[0].body else {
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
                    approved,
                    scope: ApprovalScope::Token(1),
                    timestamp,
                }
            );
        });
    }
    #[test]
    fn revoke_approval_is_a_noop_without_an_approval() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            assert_eq!(revoke_approval(&env, &owner, 1), None);
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
