//! Creator assignment event — emitted when a creator is bound to a minted NFT.
//!
//! Resolves issue #962: emit an event whenever a creator is associated with
//! an NFT containing the token ID, creator address, and ledger timestamp.
//!
//! # Event topic
//! Published under the short symbol `"creator"` so indexers can filter
//! creator assignments independently.
//!
//! # Acceptance criteria (#962)
//! - `token_id` — On-chain token identifier the creator is assigned to.
//! - `creator`  — Address of the creator wallet associated with the NFT.
//! - `timestamp` — Ledger timestamp (seconds since Unix epoch) at assignment time.
//!   (`clip_id` is also included for correlation with off-chain clip data)

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{CreatorAssignedEvent, TokenId};

/// Emit the `"creator"` assignment event after a successful creator write.
///
/// Call this **after** creator metadata is durably written so receivers are
/// guaranteed the creator is fully persisted on-chain when they process the
/// event.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `token_id`  — On-chain token ID the creator is assigned to.
/// * `creator`   — Creator wallet address.
/// * `clip_id`   — Linked off-chain clip identifier.
/// * `timestamp` — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_creator_assigned(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
    clip_id: u32,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("creator"),),
        CreatorAssignedEvent {
            token_id,
            creator: creator.clone(),
            clip_id,
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_creator_assigned_event(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
    clip_id: u32,
    timestamp: u64,
) -> CreatorAssignedEvent {
    let _ = env; // env param kept for API symmetry with emit_creator_assigned
    CreatorAssignedEvent {
        token_id,
        creator: creator.clone(),
        clip_id,
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

    // ── emit_creator_assigned ───────────────────────────────────────────────

    #[test]
    fn emit_creator_assigned_publishes_event() {
        with_contract(|env| {
            let creator = Address::generate(env);
            emit_creator_assigned(env, 7, &creator, 42, 1_700_000_000);
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
    fn multiple_creator_assignments_emit_separate_events() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            emit_creator_assigned(env, 1, &alice, 10, 100);
            emit_creator_assigned(env, 2, &bob, 20, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }

    // ── payload field coverage (acceptance criteria #962) ───────────────────

    #[test]
    fn payload_contains_token_id() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let payload = build_creator_assigned_event(env, 42, &creator, 1, 0);
            assert_eq!(payload.token_id, 42);
        });
    }

    #[test]
    fn payload_contains_creator_address() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let payload = build_creator_assigned_event(env, 1, &creator, 1, 0);
            assert_eq!(payload.creator, creator);
        });
    }

    #[test]
    fn payload_contains_timestamp() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let ts: u64 = 1_720_000_000;
            let payload = build_creator_assigned_event(env, 1, &creator, 1, ts);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn payload_contains_clip_id() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let payload = build_creator_assigned_event(env, 1, &creator, 99, 0);
            assert_eq!(payload.clip_id, 99);
        });
    }

    #[test]
    fn all_four_fields_set_in_single_call() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let ts: u64 = 1_234_567_890;
            let payload = build_creator_assigned_event(env, 77, &creator, 55, ts);
            assert_eq!(payload.token_id, 77);
            assert_eq!(payload.creator, creator);
            assert_eq!(payload.clip_id, 55);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn creator_address_preserved_across_tokens() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let p1 = build_creator_assigned_event(env, 1, &creator, 10, 100);
            let p2 = build_creator_assigned_event(env, 2, &creator, 20, 200);
            assert_eq!(p1.creator, p2.creator);
            assert_eq!(p1.creator, creator);
        });
    }
}
