//! NFT burned event — emitted whenever an NFT is permanently destroyed.
//!
//! Resolves issue #916: emit an event on every burn containing the
//! token ID, previous owner, caller, and ledger timestamp.
//!
//! # Event topic
//! Published under the short symbol `"nft_burn"` so indexers can filter
//! it independently of the legacy `"burn"` topic.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{NFTBurnedEvent, TokenId};

/// Emit the `"nft_burn"` event after an NFT has been permanently destroyed.
///
/// Call this **after** all storage removals are complete so receivers are
/// guaranteed the token no longer exists on-chain when they process the event.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `token_id`       — On-chain token ID that was burned.
/// * `previous_owner` — Address that owned the token before the burn.
/// * `caller`         — Address that initiated the burn call (may differ
///                      from `previous_owner` when an approved operator burns).
/// * `timestamp`      — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_burned(
    env: &Env,
    token_id: TokenId,
    previous_owner: &Address,
    caller: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_burn"),),
        NFTBurnedEvent {
            token_id,
            previous_owner: previous_owner.clone(),
            caller: caller.clone(),
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_nft_burned_event(
    env: &Env,
    token_id: TokenId,
    previous_owner: &Address,
    caller: &Address,
    timestamp: u64,
) -> NFTBurnedEvent {
    let _ = env; // env param kept for API symmetry with emit_nft_burned
    NFTBurnedEvent {
        token_id,
        previous_owner: previous_owner.clone(),
        caller: caller.clone(),
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

    // ── event emission ────────────────────────────────────────────────────────

    #[test]
    fn emit_publishes_exactly_one_event() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            emit_nft_burned(env, 1, &owner, &caller, 1_700_000_000);
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
    fn multiple_burns_emit_separate_events() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let caller = Address::generate(env);
            emit_nft_burned(env, 1, &owner_a, &caller, 100);
            emit_nft_burned(env, 2, &owner_b, &caller, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }

    // ── payload field coverage ────────────────────────────────────────────────

    #[test]
    fn event_payload_contains_token_id() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let payload = build_nft_burned_event(env, 42, &owner, &caller, 0);
            assert_eq!(payload.token_id, 42);
        });
    }

    #[test]
    fn event_payload_contains_previous_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let payload = build_nft_burned_event(env, 7, &owner, &caller, 0);
            assert_eq!(payload.previous_owner, owner);
        });
    }

    #[test]
    fn event_payload_contains_caller() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let payload = build_nft_burned_event(env, 3, &owner, &caller, 0);
            assert_eq!(payload.caller, caller);
        });
    }

    #[test]
    fn event_payload_contains_timestamp() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let ts: u64 = 1_720_000_000;
            let payload = build_nft_burned_event(env, 5, &owner, &caller, ts);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn caller_can_differ_from_previous_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env); // approved operator burning on behalf
            let payload = build_nft_burned_event(env, 9, &owner, &operator, 999);
            assert_ne!(payload.previous_owner, payload.caller);
            assert_eq!(payload.previous_owner, owner);
            assert_eq!(payload.caller, operator);
        });
    }

    #[test]
    fn caller_equals_owner_for_self_burn() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let payload = build_nft_burned_event(env, 11, &owner, &owner, 500);
            assert_eq!(payload.previous_owner, payload.caller);
        });
    }

    #[test]
    fn all_four_fields_set_in_single_call() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let ts = 1_234_567_890u64;
            let payload = build_nft_burned_event(env, 99, &owner, &caller, ts);
            assert_eq!(payload.token_id, 99);
            assert_eq!(payload.previous_owner, owner);
            assert_eq!(payload.caller, caller);
            assert_eq!(payload.timestamp, ts);
        });
    }
}
