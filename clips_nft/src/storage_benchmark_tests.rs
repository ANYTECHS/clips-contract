//! Storage benchmark tests — measure read/write performance (issue #1190).
//!
//! These tests measure the instruction-count cost of common storage operations
//! using the Soroban test environment.  On Stellar, CPU instructions are the
//! primary gas proxy, so instruction counts serve as deterministic, reproducible
//! performance benchmarks.
//!
//! # How it works
//! The Soroban `Env` in test mode exposes `env.budget().cpu_instruction_count()`
//! which returns the cumulative instruction count since the environment was
//! created (or last reset).  Each benchmark:
//! 1. Resets the budget counter.
//! 2. Executes the storage operation under measurement.
//! 3. Records the instruction count.
//! 4. Asserts the count is within a documented upper bound to catch regressions.
//!
//! # Results summary (documented per acceptance criteria)
//!
//! | Operation                         | Typical instructions |
//! |-----------------------------------|----------------------|
//! | Instance write (set)              | < 200 000            |
//! | Instance read (get, hit)          | < 150 000            |
//! | Persistent write (set)            | < 500 000            |
//! | Persistent read (get, hit)        | < 400 000            |
//! | Persistent read (get, miss)       | < 150 000            |
//! | Persistent has check (hit)        | < 150 000            |
//! | Persistent has check (miss)       | < 100 000            |
//! | Persistent remove                 | < 200 000            |
//! | Batch write (10 entries)          | < 5 000 000          |
//! | Batch read (10 entries, all hits) | < 4 000 000          |
//!
//! Bounds are set conservatively (2–3× observed values) to avoid flaky
//! failures from minor VM version changes while still catching major
//! regressions.  All tests are marked `#[ignore]` so they do not run in the
//! default `cargo test` invocation; run with:
//!
//! ```text
//! cargo test -p clips_nft storage_benchmark -- --ignored --nocapture
//! ```

#[cfg(test)]
mod storage_benchmark {
    use crate::types::DataKey;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    // ── Test harness ──────────────────────────────────────────────────────────

    /// Set up a contract execution environment and invoke `f` inside the
    /// contract context.  Returns whatever `f` returns.
    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── Instance storage benchmarks ───────────────────────────────────────────

    /// Benchmark: write a u32 counter to instance storage.
    #[ignore]
    #[test]
    fn bench_instance_write_u32() {
        with_contract(|env| {
            env.budget().reset_default();
            env.storage().instance().set(&DataKey::NextTokenId, &1_u32);
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] instance write u32 = {} instructions",
                instructions
            );
            assert!(
                instructions < 200_000,
                "instance write regression: {} instructions (limit 200 000)",
                instructions
            );
        });
    }

    /// Benchmark: read a u32 counter from instance storage (cache hit).
    #[ignore]
    #[test]
    fn bench_instance_read_u32_hit() {
        with_contract(|env| {
            // Pre-populate so the read is a hit.
            env.storage().instance().set(&DataKey::NextTokenId, &42_u32);
            env.budget().reset_default();
            let _val: Option<u32> = env.storage().instance().get(&DataKey::NextTokenId);
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] instance read u32 (hit) = {} instructions",
                instructions
            );
            assert!(
                instructions < 150_000,
                "instance read regression: {} instructions (limit 150 000)",
                instructions
            );
        });
    }

    // ── Persistent storage benchmarks — writes ────────────────────────────────

    /// Benchmark: write an Address to persistent storage (new key).
    #[ignore]
    #[test]
    fn bench_persistent_write_address() {
        with_contract(|env| {
            let owner = Address::generate(env);
            env.budget().reset_default();
            env.storage()
                .persistent()
                .set(&DataKey::TokenOwner(1), &owner);
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent write Address = {} instructions",
                instructions
            );
            assert!(
                instructions < 500_000,
                "persistent write regression: {} instructions (limit 500 000)",
                instructions
            );
        });
    }

    /// Benchmark: write a String (metadata URI) to persistent storage.
    #[ignore]
    #[test]
    fn bench_persistent_write_string_uri() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
            env.budget().reset_default();
            env.storage()
                .persistent()
                .set(&DataKey::Metadata(1), &uri);
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent write String URI = {} instructions",
                instructions
            );
            assert!(
                instructions < 500_000,
                "persistent write string regression: {} instructions (limit 500 000)",
                instructions
            );
        });
    }

    /// Benchmark: write a u32 clip ID to persistent storage.
    #[ignore]
    #[test]
    fn bench_persistent_write_u32() {
        with_contract(|env| {
            env.budget().reset_default();
            env.storage()
                .persistent()
                .set(&DataKey::TokenClipId(1), &7777_u32);
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent write u32 = {} instructions",
                instructions
            );
            assert!(
                instructions < 500_000,
                "persistent write u32 regression: {} instructions (limit 500 000)",
                instructions
            );
        });
    }

    // ── Persistent storage benchmarks — reads ─────────────────────────────────

    /// Benchmark: read an Address from persistent storage (cache hit).
    #[ignore]
    #[test]
    fn bench_persistent_read_address_hit() {
        with_contract(|env| {
            let owner = Address::generate(env);
            env.storage()
                .persistent()
                .set(&DataKey::TokenOwner(2), &owner);
            env.budget().reset_default();
            let _val: Option<Address> = env.storage().persistent().get(&DataKey::TokenOwner(2));
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent read Address (hit) = {} instructions",
                instructions
            );
            assert!(
                instructions < 400_000,
                "persistent read hit regression: {} instructions (limit 400 000)",
                instructions
            );
        });
    }

    /// Benchmark: read from persistent storage (key miss — key never written).
    #[ignore]
    #[test]
    fn bench_persistent_read_miss() {
        with_contract(|env| {
            env.budget().reset_default();
            let _val: Option<Address> =
                env.storage().persistent().get(&DataKey::TokenOwner(9999));
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent read (miss) = {} instructions",
                instructions
            );
            assert!(
                instructions < 150_000,
                "persistent read miss regression: {} instructions (limit 150 000)",
                instructions
            );
        });
    }

    // ── Persistent storage benchmarks — existence checks ─────────────────────

    /// Benchmark: `has` check on a key that exists (hit).
    #[ignore]
    #[test]
    fn bench_persistent_has_hit() {
        with_contract(|env| {
            let owner = Address::generate(env);
            env.storage()
                .persistent()
                .set(&DataKey::TokenOwner(3), &owner);
            env.budget().reset_default();
            let _exists = env.storage().persistent().has(&DataKey::TokenOwner(3));
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent has (hit) = {} instructions",
                instructions
            );
            assert!(
                instructions < 150_000,
                "persistent has hit regression: {} instructions (limit 150 000)",
                instructions
            );
        });
    }

    /// Benchmark: `has` check on a key that does not exist (miss).
    #[ignore]
    #[test]
    fn bench_persistent_has_miss() {
        with_contract(|env| {
            env.budget().reset_default();
            let _exists = env.storage().persistent().has(&DataKey::TokenOwner(8888));
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent has (miss) = {} instructions",
                instructions
            );
            assert!(
                instructions < 100_000,
                "persistent has miss regression: {} instructions (limit 100 000)",
                instructions
            );
        });
    }

    // ── Persistent storage benchmarks — remove ────────────────────────────────

    /// Benchmark: remove an existing entry from persistent storage.
    #[ignore]
    #[test]
    fn bench_persistent_remove() {
        with_contract(|env| {
            let owner = Address::generate(env);
            env.storage()
                .persistent()
                .set(&DataKey::TokenOwner(4), &owner);
            env.budget().reset_default();
            env.storage().persistent().remove(&DataKey::TokenOwner(4));
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] persistent remove = {} instructions",
                instructions
            );
            assert!(
                instructions < 200_000,
                "persistent remove regression: {} instructions (limit 200 000)",
                instructions
            );
        });
    }

    // ── Batch operation benchmarks ────────────────────────────────────────────

    /// Benchmark: write 10 owner entries to persistent storage (batch write).
    #[ignore]
    #[test]
    fn bench_batch_write_10_owners() {
        with_contract(|env| {
            let owners: [Address; 10] = core::array::from_fn(|_| Address::generate(env));
            env.budget().reset_default();
            for (i, owner) in owners.iter().enumerate() {
                env.storage()
                    .persistent()
                    .set(&DataKey::TokenOwner(i as u32), owner);
            }
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] batch write 10 owners = {} instructions",
                instructions
            );
            assert!(
                instructions < 5_000_000,
                "batch write regression: {} instructions (limit 5 000 000)",
                instructions
            );
        });
    }

    /// Benchmark: read 10 owner entries from persistent storage (all hits).
    #[ignore]
    #[test]
    fn bench_batch_read_10_owners_all_hits() {
        with_contract(|env| {
            // Pre-populate.
            for i in 0..10_u32 {
                let owner = Address::generate(env);
                env.storage()
                    .persistent()
                    .set(&DataKey::TokenOwner(i), &owner);
            }
            env.budget().reset_default();
            for i in 0..10_u32 {
                let _: Option<Address> =
                    env.storage().persistent().get(&DataKey::TokenOwner(i));
            }
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] batch read 10 owners (all hits) = {} instructions",
                instructions
            );
            assert!(
                instructions < 4_000_000,
                "batch read regression: {} instructions (limit 4 000 000)",
                instructions
            );
        });
    }

    /// Benchmark: write 10 metadata URIs to persistent storage.
    #[ignore]
    #[test]
    fn bench_batch_write_10_metadata_uris() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
            env.budget().reset_default();
            for i in 0..10_u32 {
                env.storage()
                    .persistent()
                    .set(&DataKey::Metadata(i), &uri);
            }
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] batch write 10 URIs = {} instructions",
                instructions
            );
            assert!(
                instructions < 5_000_000,
                "batch URI write regression: {} instructions (limit 5 000 000)",
                instructions
            );
        });
    }

    /// Benchmark: read 10 metadata URIs from persistent storage (all hits).
    #[ignore]
    #[test]
    fn bench_batch_read_10_metadata_uris_all_hits() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
            for i in 0..10_u32 {
                env.storage().persistent().set(&DataKey::Metadata(i), &uri);
            }
            env.budget().reset_default();
            for i in 0..10_u32 {
                let _: Option<String> = env.storage().persistent().get(&DataKey::Metadata(i));
            }
            let instructions = env.budget().cpu_instruction_count();
            println!(
                "[bench] batch read 10 URIs (all hits) = {} instructions",
                instructions
            );
            assert!(
                instructions < 4_000_000,
                "batch URI read regression: {} instructions (limit 4 000 000)",
                instructions
            );
        });
    }
}
