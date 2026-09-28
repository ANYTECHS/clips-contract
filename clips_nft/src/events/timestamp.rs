//! Reusable event helper for standardizing timestamps across events.
//!
//! Provides [`LedgerTimestamp`] — a small wrapper around the ledger
//! timestamp so every event consistently carries a time reference that
//! indexers can rely on.
//!
//! # Behavior
//!
//! - `LedgerTimestamp::now(env)` reads the current ledger timestamp.
//! - `LedgerTimestamp::raw(ts)` wraps a caller-supplied value.
//! - `as_u64()` extracts the inner `u64` for embedding in payloads.
//!
//! Using this wrapper instead of raw `u64` makes the contract's
//! timestamp policy explicit and testable.

use soroban_sdk::{contracttype, Env};

#[cfg(test)]
use soroban_sdk::testutils::Ledger;

/// Standardized ledger timestamp for events.
///
/// Wraps a `u64` unix timestamp sourced from `env.ledger().timestamp()`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerTimestamp(pub u64);

impl LedgerTimestamp {
    /// Capture the current ledger timestamp.
    ///
    /// Returns a `LedgerTimestamp` whose inner value equals
    /// `env.ledger().timestamp()`.
    pub fn now(env: &Env) -> Self {
        LedgerTimestamp(env.ledger().timestamp())
    }

    /// Wrap a raw `u64` timestamp.
    ///
    /// Useful in tests or when the timestamp is supplied externally.
    pub fn raw(ts: u64) -> Self {
        LedgerTimestamp(ts)
    }

    /// Extract the inner `u64` value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Convenience: extract the current ledger timestamp as a plain `u64`.
///
/// For call sites that don't need the wrapper type.
pub fn current_timestamp(env: &Env) -> u64 {
    env.ledger().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger;

    #[test]
    fn now_captures_current_ledger_timestamp() {
        let env = Env::default();
        env.ledger().set_timestamp(1_700_000_000);

        let ts = LedgerTimestamp::now(&env);
        assert_eq!(ts.as_u64(), 1_700_000_000);
    }

    #[test]
    fn raw_wraps_value() {
        let ts = LedgerTimestamp::raw(42);
        assert_eq!(ts.as_u64(), 42);
    }

    #[test]
    fn zero_timestamp_is_valid() {
        let ts = LedgerTimestamp::raw(0);
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn clone_is_equal() {
        let ts = LedgerTimestamp::raw(999);
        assert_eq!(ts, ts.clone());
    }

    #[test]
    fn current_timestamp_helper_matches_direct_read() {
        let env = Env::default();
        env.ledger().set_timestamp(2_000_000_000);

        let helper = current_timestamp(&env);
        let direct = env.ledger().timestamp();
        assert_eq!(helper, direct);
    }
}
