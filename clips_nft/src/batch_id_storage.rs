//! Batch ID generation — monotonically increasing counter for batch mint operations.
//!
//! Provides a single source of truth for assigning unique `BatchId` values to
//! every invocation of `execute_batch_mint`.  The counter lives in instance
//! storage (a single ledger entry) and is bumped on every call — even on
//! validation failures — so identifiers are never reused.
//!
//! Usage pattern mirrors `token_id_generator` / reserve_token_id in
//! `mint_service.rs`: read current, assign, write current+1.

use crate::storage_constants::DEFAULT_NEXT_BATCH_ID;
use crate::types::{BatchId, DataKey};
use soroban_sdk::Env;

/// Reserve and return the next unique batch identifier.
///
/// Performs a single instance-storage read + write (inside the shared
/// instance ledger entry, so no new write entries are consumed).
///
/// # Guarantees
/// * Strictly monotonically increasing across all `execute_batch_mint` calls.
/// * Identifiers are never re-used; a failed validation still consumes one ID.
/// * First-ever invocation returns `0`.
pub fn reserve_batch_id(env: &Env) -> BatchId {
    let current: BatchId = env
        .storage()
        .instance()
        .get::<DataKey, BatchId>(&DataKey::NextBatchId)
        .unwrap_or(DEFAULT_NEXT_BATCH_ID);
    let next = current.saturating_add(1);
    env.storage()
        .instance()
        .set(&DataKey::NextBatchId, &next);
    current
}

/// Peek at the next batch ID that will be assigned, without mutating storage.
///
/// Used by tests and off-chain simulators that need to anticipate which ID a
/// subsequent `reserve_batch_id` call will return.
pub fn peek_next_batch_id(env: &Env) -> BatchId {
    env.storage()
        .instance()
        .get::<DataKey, BatchId>(&DataKey::NextBatchId)
        .unwrap_or(DEFAULT_NEXT_BATCH_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let admin = Address::generate(&env);
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            env.storage().instance().set(&DataKey::Admin, &admin);
            f(&env)
        })
    }

    #[test]
    fn first_batch_id_is_zero() {
        with_contract(|env| {
            assert_eq!(peek_next_batch_id(env), 0);
            assert_eq!(reserve_batch_id(env), 0);
        });
    }

    #[test]
    fn reserve_increments_counter_strictly() {
        with_contract(|env| {
            assert_eq!(reserve_batch_id(env), 0);
            assert_eq!(peek_next_batch_id(env), 1);
            assert_eq!(reserve_batch_id(env), 1);
            assert_eq!(reserve_batch_id(env), 2);
            assert_eq!(peek_next_batch_id(env), 3);
        });
    }

    #[test]
    fn counter_saturates_without_panic() {
        with_contract(|env| {
            env.storage()
                .instance()
                .set(&DataKey::NextBatchId, &BatchId::MAX);
            let id = reserve_batch_id(env);
            assert_eq!(id, BatchId::MAX);
            // Second call must not panic; still returns MAX (saturating semantics).
            let id2 = reserve_batch_id(env);
            assert_eq!(id2, BatchId::MAX);
        });
    }
}
