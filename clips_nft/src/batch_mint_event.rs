//! Batch-mint-completed event — emitted once after every fully successful
//! batch mint operation.
//!
//! Resolves issue #697: emit a single `"btch_done"` event summarising the
//! outcome of a successful `execute_batch_mint` call so off-chain indexers,
//! wallets, and analytics pipelines can track batches without scanning every
//! individual `"mint"` event.
//!
//! # Event topic
//! `"btch_done"` — 9 characters, exactly at the [`soroban_sdk::symbol_short`]
//! limit.
//!
//! # Event data
//! [`BatchMintCompletedEvent`] — batch ID, number of NFTs minted, recipient
//! address, and ledger timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{BatchId, BatchMintCompletedEvent};

/// Emit the `"btch_done"` event after a fully successful batch mint.
///
/// Must be called **only** when the entire batch has been committed to storage
/// without error.  Receiving this event guarantees that all `minted_count`
/// tokens are queryable on-chain with the `recipient` address as owner.
///
/// # Arguments
/// * `env`          — Contract execution environment.
/// * `batch_id`     — Monotonically increasing identifier for this batch.
/// * `minted_count` — Number of NFTs created in this batch.
/// * `recipient`    — Address that received ownership of every minted token.
/// * `timestamp`    — Ledger timestamp in seconds since the Unix epoch.
#[allow(deprecated)]
pub fn emit_batch_mint_completed(
    env: &Env,
    batch_id: BatchId,
    minted_count: u32,
    recipient: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("btch_done"),),
        BatchMintCompletedEvent {
            batch_id,
            minted_count,
            recipient: recipient.clone(),
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BatchMintCompletedEvent;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    /// Emitting the event produces exactly one entry in the event log.
    #[test]
    fn emit_batch_mint_completed_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            emit_batch_mint_completed(&env, 1, 3, &recipient, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    /// All fields in the emitted event must exactly match the supplied arguments.
    #[test]
    fn emit_batch_mint_completed_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            let batch_id: BatchId = 42;
            let minted_count: u32 = 5;
            let timestamp: u64 = 1_720_000_000;

            emit_batch_mint_completed(&env, batch_id, minted_count, &recipient, timestamp);

            let all = env.events().all();
            assert_eq!(all.events().len(), 1);
        });
    }

    /// A batch of a single NFT (minted_count = 1) must still emit the event.
    #[test]
    fn emit_batch_mint_completed_single_item_batch() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            emit_batch_mint_completed(&env, 7, 1, &recipient, 1_700_000_001);

            let all = env.events().all();
            assert_eq!(all.events().len(), 1);
        });
    }

    /// Multiple calls each produce a distinct event; event count matches
    /// the number of calls.
    #[test]
    fn emit_batch_mint_completed_multiple_calls_produce_distinct_events() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let r1 = Address::generate(&env);
            let r2 = Address::generate(&env);

            emit_batch_mint_completed(&env, 1, 3, &r1, 1_700_000_000);
            emit_batch_mint_completed(&env, 2, 5, &r2, 1_700_000_001);

            let all = env.events().all();
            assert_eq!(all.events().len(), 2);
        });
    }

    /// No event is emitted when the function is never called (sanity check).
    #[test]
    fn no_event_emitted_without_calling_function() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
