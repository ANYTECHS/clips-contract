//! Approval Granted event — emitted when token or operator approval is granted.
//!
//! Emits an event to track when approvals are given on tokens or globally.

use soroban_sdk::{Address, Env};

use crate::event_topics::TOPIC_APPROVAL;
use crate::types::{ApprovalGrantedEvent, ApprovalScope, TokenId};

/// Emit the `"appr_grnt"` event when an approval is granted.
///
/// If `token_id` is `Some`, it is a single-token approval. Otherwise it is an
/// operator approval for all tokens owned by `owner`.
pub fn emit_approval_granted(
    env: &Env,
    owner: &Address,
    approved: &Address,
    token_id: Option<TokenId>,
) {
    env.events().publish(
        ApprovalGrantedEvent {
            owner: owner.clone(),
            approved: approved.clone(),
            scope: token_id.map_or(ApprovalScope::AllTokens, ApprovalScope::Token),
            timestamp: env.ledger().timestamp(),
        },
    );
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

    fn assert_approval_granted_event(
        env: &Env,
        owner: Address,
        approved: Address,
        scope: ApprovalScope,
        timestamp: u64,
    ) {
        let events = env.events().all();
        assert_eq!(events.events().len(), 1);
        let ContractEventBody::V0(event) = &events.events()[0].body else {
            panic!("expected a V0 contract event");
        };
        assert_eq!(event.topics.len(), 1);
        assert_eq!(
            Symbol::try_from_val(env, &event.topics[0]),
            Ok(TOPIC_APPROVAL)
        );
        assert_eq!(
            ApprovalGrantedEvent::try_from_val(env, &event.data).unwrap(),
            ApprovalGrantedEvent {
                owner,
                approved,
                scope,
                timestamp,
            }
        );
    }

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    #[test]
    fn token_approval_granted_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let approved = Address::generate(&env);
            let timestamp = 1_700_000_000;
            env.ledger().set_timestamp(timestamp);

            emit_approval_granted(&env, &owner, &approved, Some(1));
            assert_approval_granted_event(
                &env,
                owner,
                approved,
                ApprovalScope::Token(1),
                timestamp,
            );
        });
    }

    #[test]
    fn operator_approval_granted_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let approved = Address::generate(&env);
            let timestamp = 1_700_000_001;
            env.ledger().set_timestamp(timestamp);
            emit_approval_granted(&env, &owner, &approved, None);
            assert_approval_granted_event(
                &env,
                owner,
                approved,
                ApprovalScope::AllTokens,
                timestamp,
            );
        });
    }
}
