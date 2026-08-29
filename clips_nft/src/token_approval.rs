//! Token approval storage.
//!
//! Tracks which address is approved to transfer a specific token on behalf of
//! its owner (single-token approval, analogous to ERC-721 `approve`).
//!
//! # Storage
//! Key: `DataKey::Approval(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::approval_revoked_event;
use crate::types::DataKey;

/// Persist an approval: `approved` may transfer `token_id`.
pub fn save_approval(env: &Env, token_id: u32, approved: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Approval(token_id), approved);
    if let Ok(owner) = crate::token_owner_storage::get_owner(env, token_id) {
        crate::approval_granted_event::emit_approval_granted(env, &owner, approved, Some(token_id));
    }
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
    fn revoke_approval_removes_and_emits() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let approved = Address::generate(&env);
            save_approval(&env, 1, &approved);

            assert_eq!(revoke_approval(&env, &owner, 1), Some(approved));
            assert_eq!(get_approval(&env, 1), None);
            assert_eq!(env.events().all().events().len(), 1);
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
