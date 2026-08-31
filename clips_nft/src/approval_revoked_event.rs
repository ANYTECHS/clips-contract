//! Approval-revoked event (issue #931).
//!
//! Emitted whenever an NFT approval or an operator permission is withdrawn, so
//! wallets and marketplaces can drop a stale spending permission without
//! re-reading storage.
//!
//! # Event topic
//! `"aprv_rvk"` — within the 9-character limit for [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`ApprovalRevokedEvent`] — owner, the account that lost the approval, the
//! scope (a single token ID or every token owned by `owner`), and the ledger
//! timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{ApprovalRevokedEvent, ApprovalScope, TokenId};

/// Emit the `"aprv_rvk"` event for a revoked single-token approval.
///
/// Must be called **after** the approval has been removed from storage, so
/// receiving the event guarantees the permission is already gone on-chain.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `owner`     — Token owner whose approval was revoked.
/// * `approved`  — Account that lost the approval.
/// * `token_id`  — Token the approval covered.
/// * `timestamp` — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_token_approval_revoked(
    env: &Env,
    owner: &Address,
    approved: &Address,
    token_id: TokenId,
    timestamp: u64,
) {
    emit_approval_revoked(
        env,
        owner,
        approved,
        ApprovalScope::Token(token_id),
        timestamp,
    );
}

/// Emit the `"aprv_rvk"` event for a revoked operator (approve-for-all) permission.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `owner`     — Owner whose operator permission was revoked.
/// * `operator`  — Operator that lost the permission.
/// * `timestamp` — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_operator_approval_revoked(
    env: &Env,
    owner: &Address,
    operator: &Address,
    timestamp: u64,
) {
    emit_approval_revoked(env, owner, operator, ApprovalScope::AllTokens, timestamp);
}

/// Emit the `"aprv_rvk"` event for an arbitrary approval scope.
pub fn emit_approval_revoked(
    env: &Env,
    owner: &Address,
    approved: &Address,
    scope: ApprovalScope,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("aprv_rvk"),),
        ApprovalRevokedEvent {
            owner: owner.clone(),
            approved: approved.clone(),
            scope,
            timestamp,
        },
    );
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
    fn token_approval_revoked_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let approved = Address::generate(&env);
            emit_token_approval_revoked(&env, &owner, &approved, 7, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn operator_approval_revoked_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);
            emit_operator_approval_revoked(&env, &owner, &operator, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn scopes_are_distinguishable() {
        assert_ne!(ApprovalScope::Token(1), ApprovalScope::AllTokens);
        assert_ne!(ApprovalScope::Token(1), ApprovalScope::Token(2));
    }

    #[test]
    fn no_event_emitted_without_calling_function() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
