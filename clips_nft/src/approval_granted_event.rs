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
    fn token_approval_granted_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);
            emit_approval_granted(&env, &owner, &operator, Some(1));
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn operator_approval_granted_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let owner = Address::generate(&env);
            let operator = Address::generate(&env);
            emit_approval_granted(&env, &owner, &operator, None);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
}
