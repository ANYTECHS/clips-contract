//! Contract pause / unpause events (issues #933, #934).
//!
//! Emitted whenever an administrator halts or resumes contract execution, so
//! off-chain monitors can react to an emergency stop without polling the
//! [`crate::pause_state`] storage entry.
//!
//! # Event topics
//! - `"ctr_pause"` — contract paused.
//! - `"ctr_unpse"` — contract resumed.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{ContractPausedEvent, ContractUnpausedEvent};

/// Emit the `"ctr_pause"` event after the contract has been paused.
///
/// Must be called **after** the pause state is persisted, so receiving the
/// event guarantees the contract is already halted on-chain.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `admin`     — Administrator that performed the pause.
/// * `reason`    — Optional free-text reason recorded with the pause.
/// * `timestamp` — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_contract_paused(env: &Env, admin: &Address, reason: Option<String>, timestamp: u64) {
    env.events().publish(
        (symbol_short!("ctr_pause"),),
        ContractPausedEvent {
            admin: admin.clone(),
            reason,
            timestamp,
        },
    );
}

/// Emit the `"ctr_unpse"` event after the contract has been resumed.
///
/// Must be called **after** the pause state is cleared.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `admin`     — Administrator that lifted the pause.
/// * `timestamp` — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_contract_unpaused(env: &Env, admin: &Address, timestamp: u64) {
    env.events().publish(
        (symbol_short!("ctr_unpse"),),
        ContractUnpausedEvent {
            admin: admin.clone(),
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
    fn emit_contract_paused_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            emit_contract_paused(&env, &admin, None, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_contract_paused_carries_optional_reason() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            let reason = String::from_str(&env, "oracle outage");
            emit_contract_paused(&env, &admin, Some(reason), 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_contract_unpaused_publishes_one_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let admin = Address::generate(&env);
            emit_contract_unpaused(&env, &admin, 1_700_000_001);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn no_event_emitted_without_calling_function() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
