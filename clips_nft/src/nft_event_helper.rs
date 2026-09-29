//! Reusable NFT event helper for emitting lifecycle events.
//!
//! Resolves issue #909: provides a single, reusable function for emitting
//! events related to NFT lifecycle operations (mint, transfer, burn, etc.).
//!
//! # Usage
//! Call `emit_nft_event` with the token ID, event topic, and serialized
//! event data. The helper handles topic construction and emission.
//!
//! # Topic constants
//! All topics used here are imported from [`crate::event_topics`] so the
//! helper never publishes an ad-hoc `symbol_short!` label.

use soroban_sdk::{Address, Env, Symbol, Val};

use crate::event_topics::TOPIC_NFT_EVENT;
use crate::types::TokenId;

/// Emit an NFT lifecycle event with a token ID and arbitrary data.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `token_id`  — On-chain token ID this event relates to.
/// * `topic`     — Event topic symbol (use constants from `event_topics`).
/// * `data`      — Event payload as a Soroban `Val` (typically a struct or tuple).
///
/// # Event structure
/// The emitted event has two topic slots:
/// 1. `"nft_event"` — generic prefix for NFT-related events
/// 2. `topic`        — specific event type (e.g., `"nft_mint"`, `"nft_xfer"`)
///
/// This two-level topic structure lets indexers filter by `nft_event` for all
/// NFT activity, or by the specific topic for a particular operation type.
pub fn emit_nft_event<T: Into<Val>>(env: &Env, token_id: TokenId, topic: Symbol, data: T) {
    env.events()
        .publish((TOPIC_NFT_EVENT, topic), data.into());
}

/// Emit an NFT lifecycle event with token ID, sender, and recipient addresses.
///
/// Convenience wrapper for the common case where the event involves two
/// addresses (e.g., transfer from sender to recipient).
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `token_id`  — On-chain token ID this event relates to.
/// * `topic`     — Event topic symbol.
/// * `sender`    — Address initiating or from whom the action originates.
/// * `recipient` — Address receiving or to whom the action is directed.
/// * `timestamp` — Ledger timestamp in seconds.
pub fn emit_nft_address_event(
    env: &Env,
    token_id: TokenId,
    topic: Symbol,
    sender: &Address,
    recipient: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (TOPIC_NFT_EVENT, topic),
        (token_id, sender.clone(), recipient.clone(), timestamp),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use crate::event_topics::{TOPIC_BURN, TOPIC_MINT, TOPIC_TRANSFER};
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
    fn emit_nft_event_publishes_one_event() {
        with_contract(|env| {
            emit_nft_event(env, 1, TOPIC_MINT, 42_i32);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_nft_address_event_publishes_one_event() {
        with_contract(|env| {
            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            emit_nft_address_event(env, 1, TOPIC_TRANSFER, &sender, &recipient, 1000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn multiple_events_emit_independently() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            emit_nft_address_event(env, 1, TOPIC_MINT, &a, &b, 100);
            emit_nft_address_event(env, 2, TOPIC_TRANSFER, &a, &b, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }

    #[test]
    fn generic_emit_with_numeric_payload() {
        with_contract(|env| {
            emit_nft_event(env, 5, TOPIC_BURN, 42_i32);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
}
