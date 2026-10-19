//! Listing expiration validator (issue #1201).
//!
//! Validates listing expiration timestamps before they are accepted by the
//! marketplace. A value of `0` means the listing never expires and is always
//! valid. Any non-zero value is validated against the current ledger timestamp
//! plus a minimum listing duration to ensure listings are not created or
//! updated with an expiration that has already passed or is too soon to
//! settle reliably.
//!
//! # Acceptance criteria
//! - Reject invalid expiration values.
//! - Prevent already-expired listings.
//! - Validate expiration against current contract time.
//! - Add timestamp tests.
//!
//! # Errors
//! - [`Error::InvalidConfig`] — expiration is in the past or expires too soon.

use soroban_sdk::Env;

use crate::types::Error;

/// Minimum listing lifetime in seconds.
///
/// Any non-zero expiration timestamp must be at least this many seconds in the
/// future from the current ledger time. Listings expiring sooner than this
/// minimum window are rejected at creation time so they can actually be settled
/// on-chain before they expire.
///
/// Value: 1 hour (3 600 seconds), matching [`crate::marketplace::listing_validator::MIN_LISTING_DURATION_SECS`].
pub const MIN_LISTING_DURATION_SECS: u64 = 3_600;

/// Return `true` when `expires_at` names a moment that has already passed or
/// is too soon for the listing to be reliably settled.
///
/// - `0` always returns `false` (never-expiring listing).
/// - Any other value is compared against `now + MIN_LISTING_DURATION_SECS`.
///
/// # Arguments
/// * `now`        — Current ledger timestamp (seconds since Unix epoch).
/// * `expires_at` — Requested expiration timestamp; `0` = never expires.
pub fn is_expired(now: u64, expires_at: u64) -> bool {
    expires_at != 0 && expires_at < now.saturating_add(MIN_LISTING_DURATION_SECS)
}

/// Validate a listing expiration timestamp against the current contract time.
///
/// Reads the ledger timestamp and rejects any non-zero expiration that falls
/// within the minimum listing window.
///
/// # Arguments
/// * `env`        — Contract execution environment (provides ledger timestamp).
/// * `expires_at` — Requested expiration timestamp; `0` = never expires.
///
/// # Errors
/// - [`Error::InvalidConfig`] when `expires_at != 0 && expires_at < now + MIN_LISTING_DURATION_SECS`.
///
/// # Examples
/// ```rust,ignore
/// // Never-expiring listing — always valid.
/// listing_expiration_validator::validate_listing_expiration(&env, 0)?;
///
/// // Expiration at least one hour from now — valid.
/// let future = env.ledger().timestamp() + 7_200;
/// listing_expiration_validator::validate_listing_expiration(&env, future)?;
///
/// // Already-expired timestamp — rejected.
/// listing_expiration_validator::validate_listing_expiration(&env, 1)?;  // Err(InvalidConfig)
/// ```
pub fn validate_listing_expiration(env: &Env, expires_at: u64) -> Result<(), Error> {
    let now = env.ledger().timestamp();
    if is_expired(now, expires_at) {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Ledger as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ─── is_expired helper ────────────────────────────────────────────────────

    #[test]
    fn never_expires_is_never_expired() {
        // 0 = never expires; should always return false regardless of now.
        assert!(!is_expired(0, 0));
        assert!(!is_expired(u64::MAX, 0));
        assert!(!is_expired(1_000_000, 0));
    }

    #[test]
    fn expiration_equal_to_now_plus_minimum_is_valid() {
        let now: u64 = 1_000_000;
        // Exactly at the minimum boundary — not expired.
        assert!(!is_expired(now, now + MIN_LISTING_DURATION_SECS));
    }

    #[test]
    fn expiration_one_second_before_minimum_is_expired() {
        let now: u64 = 1_000_000;
        // One second short of the minimum window — expired.
        assert!(is_expired(now, now + MIN_LISTING_DURATION_SECS - 1));
    }

    #[test]
    fn expiration_in_past_is_expired() {
        let now: u64 = 1_000_000;
        assert!(is_expired(now, now - 1));
    }

    #[test]
    fn expiration_equal_to_now_is_expired() {
        let now: u64 = 1_000_000;
        // Expiration exactly at current time — already past minimum window.
        assert!(is_expired(now, now));
    }

    #[test]
    fn expiration_well_in_future_is_valid() {
        let now: u64 = 1_000_000;
        assert!(!is_expired(now, now + 86_400)); // 24 hours from now
    }

    #[test]
    fn saturating_add_does_not_overflow() {
        // When now is near u64::MAX, saturating_add should not panic.
        let now = u64::MAX - 100;
        // Any non-zero expires_at will be < now.saturating_add(MIN_...) = u64::MAX
        assert!(is_expired(now, now + 50)); // 50 < saturating sum (MAX)
    }

    // ─── validate_listing_expiration ──────────────────────────────────────────

    #[test]
    fn zero_expiration_always_valid() {
        with_contract(|env| {
            assert!(validate_listing_expiration(env, 0).is_ok());
        });
    }

    #[test]
    fn valid_future_expiration_accepted() {
        with_contract(|env| {
            let now = env.ledger().timestamp();
            let future = now + MIN_LISTING_DURATION_SECS + 3_600;
            assert!(validate_listing_expiration(env, future).is_ok());
        });
    }

    #[test]
    fn exact_minimum_boundary_is_accepted() {
        with_contract(|env| {
            let now = env.ledger().timestamp();
            assert!(
                validate_listing_expiration(env, now + MIN_LISTING_DURATION_SECS).is_ok()
            );
        });
    }

    #[test]
    fn one_second_before_minimum_is_rejected() {
        with_contract(|env| {
            let now = env.ledger().timestamp();
            let too_soon = now + MIN_LISTING_DURATION_SECS - 1;
            assert_eq!(
                validate_listing_expiration(env, too_soon),
                Err(Error::InvalidConfig)
            );
        });
    }

    #[test]
    fn past_expiration_is_rejected() {
        with_contract(|env| {
            // Ledger timestamp starts at 0 in tests; any small non-zero value
            // is already "expired" relative to the minimum duration.
            assert_eq!(
                validate_listing_expiration(env, 1),
                Err(Error::InvalidConfig)
            );
        });
    }

    #[test]
    fn current_timestamp_expiration_is_rejected() {
        with_contract(|env| {
            let now = env.ledger().timestamp();
            assert_eq!(
                validate_listing_expiration(env, now),
                Err(Error::InvalidConfig)
            );
        });
    }
}
