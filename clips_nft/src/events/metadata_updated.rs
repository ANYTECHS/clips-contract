//! Metadata updated event — centralized helper (issue #961).
//!
//! Mirrors `crate::metadata_updated_event` but lives under the centralized
//! `events::metadata_updated` namespace so callers can use
//! `events::metadata_updated::emit_metadata_updated` without importing the
//! standalone module. The standalone `metadata_updated_event` module is retained
//! for backward compatibility.
//!
//! # Event topic
//! Published under the short symbol `"meta_upd"` so indexers can filter
//! it independently.
//!
//! # Acceptance criteria (#961)
//! - `token_id`     — On-chain token identifier whose metadata changed.
//! - `previous_uri` — Previous metadata reference before the update.
//! - `new_uri`      — New metadata reference after the update.
//! - `timestamp`    — Ledger timestamp (seconds since Unix epoch) at update time.
//!   (`updater` is also included for auditability)

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{MetadataUpdatedEvent, TokenId};

/// Emit the `"meta_upd"` event after NFT metadata has changed.
///
/// Call this **after** all storage writes are complete.
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
/// Used by tests to verify every required field is populated correctly.
pub fn build_metadata_updated_event(
    env: &Env,
    token_id: TokenId,
    previous_uri: &String,
    new_uri: &String,
    updater: &Address,
    timestamp: u64,
) -> MetadataUpdatedEvent {
    let _ = env;
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
}
