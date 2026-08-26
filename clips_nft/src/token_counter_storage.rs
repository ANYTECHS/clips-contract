//! Token counter storage — maintains the total number of NFTs minted.
//!
//! Provides a single monotonically-increasing counter that is incremented
//! on every successful mint.  Satisfies issue #504: increment counter,
//! read counter, reset during tests.
//!
//! # Storage
//! Key: `DataKey::TokenCounter` (instance storage)

use soroban_sdk::Env;

use crate::storage_constants::DEFAULT_TOKEN_COUNTER;
use crate::types::{DataKey, Error};

// ── Read ──────────────────────────────────────────────────────────────────────

/// Return the current total number of NFTs minted.
///
/// Defaults to  when no mint has occurred yet.
pub fn read_token_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::TokenCounter)
        .unwrap_or(DEFAULT_TOKEN_COUNTER)
}

// ── Increment ─────────────────────────────────────────────────────────────────

/// Increment the token counter by one and persist the new value.
///
/// Call this once per successful mint, after all other writes complete.
///
/// # Errors
/// Returns [`Error::SupplyOverflow`] when the counter would exceed `u32::MAX`.
pub fn increment_token_count(env: &Env) -> Result<u32, Error> {
    let next = read_token_count(env)
        .checked_add(1)
        .ok_or(Error::SupplyOverflow)?;
    env.storage().instance().set(&DataKey::TokenCounter, &next);
    Ok(next)
}

// ── Reset (test helper) ───────────────────────────────────────────────────────

/// Reset the counter to .
///
/// Intended for test setup and migration tooling only — do not call from
/// production mint paths.
pub fn reset_token_count(env: &Env, value: u32) {
    env.storage().instance().set(&DataKey::TokenCounter, &value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::Env;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── read_token_count ──────────────────────────────────────────────────────

    #[test]
    fn read_returns_zero_before_any_mint() {
        with_contract(|env| {
            assert_eq!(read_token_count(env), 0);
        });
    }

    #[test]
    fn read_reflects_incremented_value() {
        with_contract(|env| {
            increment_token_count(env).unwrap();
            assert_eq!(read_token_count(env), 1);
        });
    }

    // ── increment_token_count ─────────────────────────────────────────────────

    #[test]
    fn increment_starts_from_zero_and_returns_new_value() {
        with_contract(|env| {
            assert_eq!(increment_token_count(env).unwrap(), 1);
        });
    }

    #[test]
    fn increment_is_strictly_monotonic() {
        with_contract(|env| {
            assert_eq!(increment_token_count(env).unwrap(), 1);
            assert_eq!(increment_token_count(env).unwrap(), 2);
            assert_eq!(increment_token_count(env).unwrap(), 3);
            assert_eq!(read_token_count(env), 3);
        });
    }

    #[test]
    fn increment_returns_overflow_error_at_u32_max() {
        with_contract(|env| {
            reset_token_count(env, u32::MAX);
            assert_eq!(increment_token_count(env), Err(Error::SupplyOverflow));
            // Counter must not change on overflow.
            assert_eq!(read_token_count(env), u32::MAX);
        });
    }

    // ── reset_token_count ─────────────────────────────────────────────────────

    #[test]
    fn reset_to_zero_clears_counter() {
        with_contract(|env| {
            increment_token_count(env).unwrap();
            increment_token_count(env).unwrap();
            reset_token_count(env, 0);
            assert_eq!(read_token_count(env), 0);
        });
    }

    #[test]
    fn reset_to_arbitrary_value() {
        with_contract(|env| {
            reset_token_count(env, 42);
            assert_eq!(read_token_count(env), 42);
        });
    }

    #[test]
    fn increment_after_reset_continues_from_reset_value() {
        with_contract(|env| {
            reset_token_count(env, 10);
            assert_eq!(increment_token_count(env).unwrap(), 11);
            assert_eq!(read_token_count(env), 11);
        });
    }

    #[test]
    fn multiple_resets_work_independently() {
        with_contract(|env| {
            reset_token_count(env, 5);
            assert_eq!(read_token_count(env), 5);
            reset_token_count(env, 100);
            assert_eq!(read_token_count(env), 100);
            reset_token_count(env, 0);
            assert_eq!(read_token_count(env), 0);
        });
    }
}
