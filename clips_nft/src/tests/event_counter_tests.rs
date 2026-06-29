#![cfg(test)]

use soroban_sdk::Env;

use crate::{event_counter_storage, ClipCashNFT};

fn setup_env() -> Env {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClipCashNFT);
    env.as_contract(&contract_id, || {});
    env
}

#[test]
fn counter_starts_at_zero() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClipCashNFT);
    env.as_contract(&contract_id, || {
        assert_eq!(event_counter_storage::get_event_counter(&env, 0), 0);
        assert_eq!(event_counter_storage::get_event_counter(&env, 1), 0);
        assert_eq!(event_counter_storage::get_event_counter(&env, 99), 0);
    });
}

#[test]
fn increment_returns_new_count() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClipCashNFT);
    env.as_contract(&contract_id, || {
        assert_eq!(event_counter_storage::increment_event_counter(&env, 0), 1);
        assert_eq!(event_counter_storage::increment_event_counter(&env, 0), 2);
        assert_eq!(event_counter_storage::increment_event_counter(&env, 0), 3);
    });
}

#[test]
fn counters_are_independent_per_event_type() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClipCashNFT);
    env.as_contract(&contract_id, || {
        event_counter_storage::increment_event_counter(&env, 1);
        event_counter_storage::increment_event_counter(&env, 1);
        event_counter_storage::increment_event_counter(&env, 2);

        assert_eq!(event_counter_storage::get_event_counter(&env, 1), 2);
        assert_eq!(event_counter_storage::get_event_counter(&env, 2), 1);
        assert_eq!(event_counter_storage::get_event_counter(&env, 3), 0);
    });
}

#[test]
fn get_after_increment_reflects_persisted_value() {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClipCashNFT);
    env.as_contract(&contract_id, || {
        for _ in 0..10 {
            event_counter_storage::increment_event_counter(&env, 42);
        }
        assert_eq!(event_counter_storage::get_event_counter(&env, 42), 10);
    });
}
