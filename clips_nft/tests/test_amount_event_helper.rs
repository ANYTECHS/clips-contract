//! Integration and unit tests for the financial Amount Event Helper (issue #911).
//!
//! Verifies:
//! - Standard topic label `"amount"` and `"amt_trns"` emission.
//! - Exact payload serialization matching `AmountEvent` and `AmountTransferredEvent`.
//! - Accurate handling of sender, recipient, asset address, amount, and timestamp.
//! - Default ledger timestamp resolution in `_now` helpers.
//! - Boundary values (zero, small, standard, large amounts) and multiple assets.

#![cfg(test)]

use clips_nft::{
    events::{
        emit_amount, emit_amount_now, emit_amount_transferred, emit_amount_transferred_now,
        AmountEvent, AmountTransferredEvent,
    },
    ClipsNftContract,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env, Symbol, TryFromVal,
};

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env, Address) -> R,
{
    let env = Env::default();
    let contract_id = env.register(ClipsNftContract, ());
    env.as_contract(&contract_id, || {
        ClipsNftContract::init(env.clone(), Address::generate(&env));
        f(&env, contract_id.clone())
    })
}

fn event_count(env: &Env) -> usize {
    env.events().all().events().len()
}

#[test]
fn test_emit_amount_publishes_expected_topic_and_fields() {
    with_contract(|env, _contract_id| {
        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let asset = Address::generate(env);
        let amount = 150_000_000i128; // 15 XLM in stroops
        let timestamp = 1726050000u64;

        emit_amount(env, amount, &asset, &sender, &recipient, timestamp);

        assert_eq!(event_count(env), 1, "exactly one event should be published");

        let all = env.events().all();
        assert!(
            all.events().iter().any(|e| {
                let soroban_sdk::xdr::ContractEventBody::V0(v0) = &e.body;
                if v0.topics.len() == 1 {
                    if let Ok(sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                        if sym == symbol_short!("amount") {
                            if let Ok(parsed) = AmountEvent::try_from_val(env, &v0.data) {
                                return parsed.amount == amount
                                    && parsed.asset == asset
                                    && parsed.sender == sender
                                    && parsed.recipient == recipient
                                    && parsed.timestamp == timestamp;
                            }
                        }
                    }
                }
                false
            }),
            "expected AmountEvent not found in published events"
        );
    });
}

#[test]
fn test_emit_amount_now_derives_ledger_timestamp() {
    with_contract(|env, _contract_id| {
        let expected_timestamp = 1726099999u64;
        env.ledger().set_timestamp(expected_timestamp);

        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let asset = Address::generate(env);
        let amount = 42_000_000i128;

        emit_amount_now(env, amount, &asset, &sender, &recipient);

        let all = env.events().all();
        assert!(
            all.events().iter().any(|e| {
                let soroban_sdk::xdr::ContractEventBody::V0(v0) = &e.body;
                if v0.topics.len() == 1 {
                    if let Ok(sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                        if sym == symbol_short!("amount") {
                            if let Ok(parsed) = AmountEvent::try_from_val(env, &v0.data) {
                                return parsed.amount == amount
                                    && parsed.timestamp == expected_timestamp;
                            }
                        }
                    }
                }
                false
            }),
            "AmountEvent with ledger timestamp not found"
        );
    });
}

#[test]
fn test_emit_amount_transferred_publishes_expected_topic_and_fields() {
    with_contract(|env, _contract_id| {
        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let asset = Address::generate(env);
        let amount = 500_000_000i128;
        let timestamp = 1726110000u64;

        emit_amount_transferred(env, amount, &asset, &sender, &recipient, timestamp);

        let all = env.events().all();
        assert!(
            all.events().iter().any(|e| {
                let soroban_sdk::xdr::ContractEventBody::V0(v0) = &e.body;
                if v0.topics.len() == 1 {
                    if let Ok(sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                        if sym == symbol_short!("amt_trns") {
                            if let Ok(parsed) = AmountTransferredEvent::try_from_val(env, &v0.data)
                            {
                                return parsed.amount == amount
                                    && parsed.asset == asset
                                    && parsed.sender == sender
                                    && parsed.recipient == recipient
                                    && parsed.timestamp == timestamp;
                            }
                        }
                    }
                }
                false
            }),
            "AmountTransferredEvent not found in events"
        );
    });
}

#[test]
fn test_emit_amount_transferred_now_derives_ledger_timestamp() {
    with_contract(|env, _contract_id| {
        let test_timestamp = 1726123456u64;
        env.ledger().set_timestamp(test_timestamp);

        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let asset = Address::generate(env);
        let amount = 999_999_999i128;

        emit_amount_transferred_now(env, amount, &asset, &sender, &recipient);

        let all = env.events().all();
        assert!(
            all.events().iter().any(|e| {
                let soroban_sdk::xdr::ContractEventBody::V0(v0) = &e.body;
                if v0.topics.len() == 1 {
                    if let Ok(sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                        if sym == symbol_short!("amt_trns") {
                            if let Ok(parsed) = AmountTransferredEvent::try_from_val(env, &v0.data)
                            {
                                return parsed.amount == amount
                                    && parsed.timestamp == test_timestamp;
                            }
                        }
                    }
                }
                false
            }),
            "AmountTransferredEvent with ledger timestamp not found"
        );
    });
}

#[test]
fn test_amount_boundary_values_and_multiple_emissions() {
    with_contract(|env, _contract_id| {
        let sender = Address::generate(env);
        let recipient = Address::generate(env);
        let usdc_asset = Address::generate(env);
        let xlm_asset = Address::generate(env);

        let initial_count = event_count(env);

        // Test zero amount
        emit_amount(env, 0i128, &usdc_asset, &sender, &recipient, 100);

        // Test unit stroop amount
        emit_amount(env, 1i128, &xlm_asset, &sender, &recipient, 101);

        // Test large bounded amount (half i128 max)
        let large_amt = i128::MAX / 2;
        emit_amount_transferred(env, large_amt, &usdc_asset, &sender, &recipient, 102);

        assert_eq!(
            event_count(env),
            initial_count + 3,
            "exactly 3 financial events should be appended"
        );
    });
}

#[test]
fn test_sender_and_recipient_invariance() {
    with_contract(|env, _contract_id| {
        let alice = Address::generate(env);
        let bob = Address::generate(env);
        let asset = Address::generate(env);
        let amount = 77_000_000i128;

        // Alice sends to Bob
        emit_amount(env, amount, &asset, &alice, &bob, 200);

        // Bob sends back to Alice
        emit_amount_transferred(env, amount, &asset, &bob, &alice, 201);

        let all = env.events().all();
        let mut found_alice_to_bob = false;
        let mut found_bob_to_alice = false;

        for e in all.events().iter() {
            let soroban_sdk::xdr::ContractEventBody::V0(v0) = &e.body;
            if v0.topics.len() == 1 {
                if let Ok(sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                    if sym == symbol_short!("amount") {
                        if let Ok(parsed) = AmountEvent::try_from_val(env, &v0.data) {
                            if parsed.sender == alice && parsed.recipient == bob {
                                found_alice_to_bob = true;
                            }
                        }
                    } else if sym == symbol_short!("amt_trns") {
                        if let Ok(parsed) = AmountTransferredEvent::try_from_val(env, &v0.data) {
                            if parsed.sender == bob && parsed.recipient == alice {
                                found_bob_to_alice = true;
                            }
                        }
                    }
                }
            }
        }

        assert!(found_alice_to_bob, "Alice to Bob event not found");
        assert!(found_bob_to_alice, "Bob to Alice event not found");
    });
}
