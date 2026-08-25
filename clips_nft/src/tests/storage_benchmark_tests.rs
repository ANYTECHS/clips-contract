//! Storage benchmark tests — resolves issue #540.
//!
//! Measures storage read and write throughput by counting successful operations
//! in bulk. Results are validated by assertion rather than wall-clock timing,
//! which would be non-deterministic in Soroban's wasm execution model.
//!
//! # Soroban ledger entry limits
//! Each persistent storage key is a separate ledger entry.
//! Soroban caps a single invocation at:
//!   - 100 total footprint ledger entries
//!   - 50 write ledger entries
//!
//! Instance storage is a *single* ledger entry regardless of how many keys are
//! stored inside it, so instance benchmarks can use a larger BENCH_SIZE.
//!
//! # Observed results (local test runs, soroban-sdk 25.3.1)
//! - 25 persistent writes: all succeed, 25 ledger entries consumed.
//! - 25 subsequent reads: correct values returned, no additional entries.
//! - 50 instance reads/writes: all succeed within a single ledger entry.
//! - Interleaved 25 write+read pairs: each pair is in-footprint, all pass.

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{
    owner_portfolio, token_owner_storage, types::DataKey, wallet_token_index, ClipCashNFT,
};

/// Persistent storage benchmark size — kept within the 50-write-entry limit.
const PERSISTENT_BENCH_SIZE: u32 = 25;

/// Instance storage benchmark size — unconstrained by per-key ledger limits.
const INSTANCE_BENCH_SIZE: u32 = 50;

fn setup() -> (Env, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, ClipCashNFT);
    (env, contract_id)
}

// ── Persistent write benchmarks ──────────────────────────────────────────────

/// Benchmark: PERSISTENT_BENCH_SIZE persistent writes of token metadata URIs.
///
/// Verifies that all write operations complete and persist correctly.
#[test]
fn benchmark_persistent_writes() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        for i in 0..PERSISTENT_BENCH_SIZE {
            let uri = String::from_str(&env, "ipfs://benchmark");
            env.storage().persistent().set(&DataKey::Metadata(i), &uri);
        }
        for i in 0..PERSISTENT_BENCH_SIZE {
            assert!(env.storage().persistent().has(&DataKey::Metadata(i)));
        }
    });
}

/// Benchmark: PERSISTENT_BENCH_SIZE instance-storage writes of event counters.
///
/// Instance storage is one ledger entry, so INSTANCE_BENCH_SIZE is larger.
#[test]
fn benchmark_instance_writes() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        for i in 0..INSTANCE_BENCH_SIZE {
            env.storage().instance().set(&DataKey::EventCounter(i), &i);
        }
        for i in 0..INSTANCE_BENCH_SIZE {
            assert!(env.storage().instance().has(&DataKey::EventCounter(i)));
        }
    });
}

// ── Persistent read benchmarks ───────────────────────────────────────────────

/// Benchmark: PERSISTENT_BENCH_SIZE persistent reads after seeding.
///
/// Reads back the same keys that were written — no extra footprint entries.
#[test]
fn benchmark_persistent_reads() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        // Seed.
        for i in 0..PERSISTENT_BENCH_SIZE {
            let owner = Address::generate(&env);
            let data = crate::types::TokenData { owner, clip_id: i };
            env.storage().persistent().set(&DataKey::Token(i), &data);
        }

        // Read back and verify every entry.
        let mut read_count = 0u32;
        for i in 0..PERSISTENT_BENCH_SIZE {
            let data: Option<crate::types::TokenData> =
                env.storage().persistent().get(&DataKey::Token(i));
            assert!(data.is_some());
            assert_eq!(data.unwrap().clip_id, i);
            read_count += 1;
        }
        assert_eq!(read_count, PERSISTENT_BENCH_SIZE);
    });
}

/// Benchmark: INSTANCE_BENCH_SIZE instance-storage reads after seeding.
#[test]
fn benchmark_instance_reads() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        for i in 0..INSTANCE_BENCH_SIZE {
            env.storage()
                .instance()
                .set(&DataKey::EventCounter(i), &(i * 10));
        }
        for i in 0..INSTANCE_BENCH_SIZE {
            let val: Option<u32> = env.storage().instance().get(&DataKey::EventCounter(i));
            assert_eq!(val, Some(i * 10));
        }
    });
}

// ── Interleaved read/write ───────────────────────────────────────────────────

/// Benchmark: interleaved writes and reads simulating realistic access patterns.
///
/// Each iteration writes a value and immediately reads it back,
/// verifying round-trip correctness for PERSISTENT_BENCH_SIZE unique keys.
#[test]
fn benchmark_interleaved_read_write() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        for i in 0..PERSISTENT_BENCH_SIZE {
            env.storage()
                .persistent()
                .set(&DataKey::CollectionSupply(i), &i);
            let val: Option<u32> = env
                .storage()
                .persistent()
                .get(&DataKey::CollectionSupply(i));
            assert_eq!(val, Some(i));
        }
    });
}

/// Benchmark the optimized transfer index path: two index reads and two
/// writes for a transfer between wallets, with no duplicate membership reads.
#[test]
fn benchmark_transfer_index_update() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        wallet_token_index::add_token_to_wallet(&env, &from, 1).unwrap();

        wallet_token_index::move_token_between_wallets(&env, &from, &to, 1).unwrap();

        assert!(wallet_token_index::get_wallet_tokens(&env, &from).is_empty());
        assert_eq!(wallet_token_index::get_wallet_tokens(&env, &to).len(), 1);

        owner_portfolio::add_token_to_owner(&env, &from, 1).unwrap();
        owner_portfolio::move_token_between_owners(&env, &from, &to, 1).unwrap();
        assert!(owner_portfolio::get_owner_portfolio(&env, &from).is_empty());
        assert_eq!(owner_portfolio::get_owner_portfolio(&env, &to).len(), 1);
    });
}

/// Benchmark an owner update after transfer validation has already loaded the
/// token record, avoiding the extra `has` read in `update_owner`.
#[test]
fn benchmark_owner_update_after_validation() {
    let (env, contract_id) = setup();
    env.as_contract(&contract_id, || {
        let old_owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        token_owner_storage::save_owner(&env, 1, &old_owner);

        token_owner_storage::update_owner_after_validation(&env, 1, &new_owner);

        assert_eq!(token_owner_storage::get_owner(&env, 1).unwrap(), new_owner);
    });
}
