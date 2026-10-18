//! Metadata updated event — emitted whenever an NFT's metadata reference is updated.
//!
//! Resolves issue #961: emit an event on every metadata update containing the
//! token ID, previous metadata reference, new metadata reference, and ledger timestamp.
//!
//! # Event topic
//! Published under the short symbol `"meta_upd"` (8 chars) so indexers can filter
//! it independently of other events. The payload is [`MetadataUpdatedEvent`] which
//! carries all fields required by the acceptance criteria.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{MetadataUpdatedEvent, TokenId};

/// Emit the `"meta_upd"` event after NFT metadata has been updated.
///
/// Call this **after** all storage writes are complete so receivers are
/// guaranteed the new metadata is fully persisted on-chain when they
/// process the event.
///
/// # Arguments
/// * `env`          — Contract execution environment.
/// * `token_id`     — On-chain token ID whose metadata changed.
/// * `previous_uri` — Previous metadata reference (URI) before the update.
/// * `new_uri`      — New metadata reference (URI) after the update.
/// * `updater`      — Address that performed the update.
/// * `timestamp`    — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_metadata_updated(
    env: &Env,
    token_id: TokenId,
    previous_uri: &String,
    new_uri: &String,
    updater: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("meta_upd"),),
        MetadataUpdatedEvent {
            token_id,
            previous_uri: previous_uri.clone(),
            new_uri: new_uri.clone(),
            updater: updater.clone(),
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_metadata_updated_event(
    env: &Env,
    token_id: TokenId,
    previous_uri: &String,
    new_uri: &String,
    updater: &Address,
    timestamp: u64,
) -> MetadataUpdatedEvent {
    let _ = env; // env kept for API symmetry with emit_metadata_updated
    MetadataUpdatedEvent {
        token_id,
        previous_uri: previous_uri.clone(),
        new_uri: new_uri.clone(),
        updater: updater.clone(),
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env, String,
    };

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn emit_publishes_exactly_one_event() {
        with_contract(|env| {
            let updater = Address::generate(env);
            let prev = String::from_str(env, "ipfs://old");
            let new = String::from_str(env, "ipfs://new");
            emit_metadata_updated(env, 1, &prev, &new, &updater, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn payload_contains_all_five_fields() {
        with_contract(|env| {
            let updater = Address::generate(env);
            let prev = String::from_str(env, "ipfs://prev");
            let new = String::from_str(env, "ipfs://new");
            let ts: u64 = 1_720_000_000;
            let payload = build_metadata_updated_event(env, 42, &prev, &new, &updater, ts);
            assert_eq!(payload.token_id, 42);
            assert_eq!(payload.previous_uri, prev);
            assert_eq!(payload.new_uri, new);
            assert_eq!(payload.updater, updater);
            assert_eq!(payload.timestamp, ts);
        });
    }

    #[test]
    fn previous_and_new_uri_are_distinct() {
        with_contract(|env| {
            let updater = Address::generate(env);
            let prev = String::from_str(env, "ipfs://old");
            let new = String::from_str(env, "ipfs://new");
            let payload = build_metadata_updated_event(env, 1, &prev, &new, 100);
            assert_ne!(payload.previous_uri, payload.new_uri);
        });
    }

    #[test]
    fn multiple_updates_emit_separate_events() {
        with_contract(|env| {
            let updater = Address::generate(env);
            let prev1 = String::from_str(env, "ipfs://old1");
            let new1 = String::from_str(env, "ipfs://new1");
            let prev2 = String::from_str(env, "ipfs://old2");
            let new2 = String::from_str(env, "ipfs://new2");
            emit_metadata_updated(env, 1, &prev1, &new1, &updater, 100);
            emit_metadata_updated(env, 2, &prev2, &new2, &updater, 200);
            assert_eq!(env.events().all().events().len(), 2);
        });
    }
}
