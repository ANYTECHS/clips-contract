//! Reusable address event helper for emitting events involving Stellar addresses.
//!
//! Resolves issue #910: provides helpers for events that involve wallet
//! addresses (sender, recipient, creator, owner) with consistent serialization.
//!
//! # Design
//! Addresses are serialized consistently as `Address` values in Soroban events.
//! The helper accepts role-labeled parameters (sender, recipient, creator, owner)
//! so callers don't need to remember parameter order — each role is named
//! explicitly.

use soroban_sdk::{symbol_short, Address, Env};

/// Emit an address-centric event with sender and recipient.
///
/// Common for transfer-style events where value moves from one address to another.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `topic`     — Event topic symbol (from `event_topics`).
/// * `sender`    — Address initiating the action (source of value/action).
/// * `recipient` — Address receiving the action (destination of value/action).
/// * `amount`    — Optional amount associated with the event (use 0 if N/A).
/// * `timestamp` — Ledger timestamp in seconds.
pub fn emit_sender_recipient_event(
    env: &Env,
    topic: &str,
    sender: &Address,
    recipient: &Address,
    amount: i128,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("addr_event"),),
        (
            soroban_sdk::symbol_short!(&topic[..9.min(topic.len())]),
            sender.clone(),
            recipient.clone(),
            amount,
            timestamp,
        ),
    );
}

/// Emit an address-centric event with creator and owner.
///
/// Common for NFT lifecycle events where the creator minted and the
/// current owner holds the token.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `topic`     — Event topic symbol.
/// * `creator`   — Address that created the asset.
/// * `owner`     — Current owner address.
/// * `token_id`  — Associated token ID (use 0 if N/A).
/// * `timestamp` — Ledger timestamp in seconds.
pub fn emit_creator_owner_event(
    env: &Env,
    topic: &str,
    creator: &Address,
    owner: &Address,
    token_id: u32,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("addr_event"),),
        (
            soroban_sdk::symbol_short!(&topic[..9.min(topic.len())]),
            creator.clone(),
            owner.clone(),
            token_id,
            timestamp,
        ),
    );
}

/// Emit an address-centric event with a single address and amount.
///
/// Useful for approval, configuration, or status-change events involving
/// one primary address.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `topic`     — Event topic symbol.
/// * `address`   — The primary address involved.
/// * `amount`    — Associated amount (use 0 if N/A).
/// * `timestamp` — Ledger timestamp in seconds.
pub fn emit_single_address_event(
    env: &Env,
    topic: &str,
    address: &Address,
    amount: i128,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("addr_event"),),
        (
            soroban_sdk::symbol_short!(&topic[..9.min(topic.len())]),
            address.clone(),
            amount,
            timestamp,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, testutils::Events, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn sender_recipient_event_publishes_one_event() {
        with_contract(|env| {
            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            emit_sender_recipient_event(env, "transfer", &sender, &recipient, 100, 1000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn creator_owner_event_publishes_one_event() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            emit_creator_owner_event(env, "mint", &creator, &owner, 42, 1000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn single_address_event_publishes_one_event() {
        with_contract(|env| {
            let addr = Address::generate(env);
            emit_single_address_event(env, "approval", &addr, 0, 1000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn multiple_address_events_emit_independently() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let c = Address::generate(env);
            emit_sender_recipient_event(env, "transfer", &a, &b, 100, 100);
            emit_sender_recipient_event(env, "transfer", &b, &c, 50, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }

    #[test]
    fn sender_and_recipient_are_distinct() {
        with_contract(|env| {
            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            emit_sender_recipient_event(env, "test", &sender, &recipient, 0, 0);
            assert_ne!(sender, recipient);
        });
    }

    #[test]
    fn creator_can_equal_owner() {
        with_contract(|env| {
            let addr = Address::generate(env);
            emit_creator_owner_event(env, "self_mint", &addr, &addr, 1, 0);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
}
