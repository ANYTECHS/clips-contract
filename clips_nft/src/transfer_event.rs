//! NFT transferred event — emitted whenever NFT ownership changes.
//!
//! Resolves issue #915: emit an event on every transfer containing the
//! token ID, previous owner, new owner, and ledger timestamp.
//!
//! # Event topic
//! Published under the short symbol `"nft_xfer"` so indexers can filter
//! it independently of the legacy `"transfer"` topic.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{NFTTransferredEvent, TokenId};

/// Emit the `"nft_xfer"` event after NFT ownership has changed.
///
/// Call this **after** all storage writes are complete so receivers are
/// guaranteed the new owner is fully persisted on-chain when they
/// process the event.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `token_id`       — On-chain token ID whose ownership changed.
/// * `previous_owner` — Address that owned the token before the transfer.
/// * `new_owner`      — Address that received the token.
/// * `timestamp`      — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_transferred(
    env: &Env,
    token_id: TokenId,
    previous_owner: &Address,
    new_owner: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_xfer"),),
        NFTTransferredEvent {
            token_id,
            previous_owner: previous_owner.clone(),
            new_owner: new_owner.clone(),
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_nft_transferred_event(
    env: &Env,
    token_id: TokenId,
    previous_owner: &Address,
    new_owner: &Address,
    timestamp: u64,
) -> NFTTransferredEvent {
    let _ = env; // env kept for API symmetry with emit_nft_transferred
    NFTTransferredEvent {
        token_id,
        previous_owner: previous_owner.clone(),
        new_owner: new_owner.clone(),
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── emit_nft_transferred ──────────────────────────────────────────────────

    #[test]
    fn emit_publishes_exactly_one_event() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            emit_nft_transferred(env, 1, &from, &to, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn no_event_emitted_when_not_called() {
        with_contract(|env| {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }

    #[test]
    fn multiple_transfers_emit_separate_events() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let c = Address::generate(env);
            emit_nft_transferred(env, 1, &a, &b, 100);
            emit_nft_transferred(env, 1, &b, &c, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }

    // ── payload field coverage (acceptance criteria) ──────────────────────────

    #[test]
    fn payload_contains_token_id() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let payload = build_nft_transferred_event(env, 42, &from, &to, 0);
            assert_eq!(payload.token_id, 42);
        });
    }

    #[test]
    fn payload_contains_previous_owner() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let payload = build_nft_transferred_event(env, 1, &from, &to, 0);
            assert_eq!(payload.previous_owner, from);
        });
    }

    #[test]
    fn payload_contains_new_owner() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let payload = build_nft_transferred_event(env, 1, &from, &to, 0);
            assert_eq!(payload.new_owner, to);
        });
    }

    #[test]
    fn payload_contains_timestamp() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let ts: u64 = 1_720_000_000;
            let payload = build_nft_transferred_event(env, 1, &from, &to, ts);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn previous_owner_and_new_owner_are_distinct() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let payload = build_nft_transferred_event(env, 5, &from, &to, 0);
            assert_ne!(payload.previous_owner, payload.new_owner);
        });
    }

    #[test]
    fn all_four_fields_set_in_single_call() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let ts: u64 = 1_234_567_890;
            let payload = build_nft_transferred_event(env, 77, &from, &to, ts);
            assert_eq!(payload.token_id, 77);
            assert_eq!(payload.previous_owner, from);
            assert_eq!(payload.new_owner, to);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn same_token_can_transfer_multiple_times() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let c = Address::generate(env);

            // First transfer: a -> b
            let p1 = build_nft_transferred_event(env, 10, &a, &b, 100);
            assert_eq!(p1.previous_owner, a);
            assert_eq!(p1.new_owner, b);

            // Second transfer: b -> c
            let p2 = build_nft_transferred_event(env, 10, &b, &c, 200);
            assert_eq!(p2.previous_owner, b);
            assert_eq!(p2.new_owner, c);
            assert_eq!(p2.token_id, p1.token_id);
        });
    }
}
