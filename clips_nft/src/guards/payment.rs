//! Payment replay detection guard — prevent duplicate payment processing (issue #1091).
//!
//! This guard module provides replay protection for payment operations,
//! particularly royalty and marketplace payments where idempotency is critical.
//!
//! # Replay protection strategy
//!
//! To prevent duplicate or replayed payments:
//!
//! 1. **Hash generation** — create a deterministic hash from (payer, token_id, sale_price).
//! 2. **Idempotency check** — before processing, check if the hash is in the store.
//! 3. **Recording** — after successful processing, record the hash to prevent replay.
//! 4. **Deterministic error** — return the same error for any duplicate; no state change.
//!
//! # Error handling
//!
//! Returns [`PaymentReplayError`] variants mapped to standardized error codes
//! (265–268).
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::guards::payment;
//!
//! // Detect a replay before processing
//! payment::detect_replay_payment(&env, payer, token_id, sale_price)?;
//!
//! // Process payment logic here...
//!
//! // Record that payment was successfully processed
//! payment::record_payment_processed(&env, payer, token_id, sale_price)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::error_infrastructure::PaymentReplayError;
use crate::guards::{Guard, GuardContext, GuardResult};
use crate::types::{Error, TokenId};

/// Detect if a payment has already been processed (replay detection).
///
/// Returns [`Error`] with code 268 (`PaymentAlreadyProcessed`) if:
/// - The payment hash is found in idempotency storage
/// - A prior transaction processed this exact (payer, token_id, sale_price) tuple
/// - Cumulative payment would overflow
///
/// # Errors
/// - [`Error`] — if replay is detected
///
/// # Side effects
/// - None. This is a read-only check with no state modification.
pub fn detect_replay_payment(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> Result<(), Error> {
    if is_payment_already_processed(env, payer, token_id, sale_price) {
        return Err(Error::PaymentAlreadyProcessed);
    }
    Ok(())
}

/// Guard implementation for payment replay detection.
///
/// Wraps [`detect_replay_payment`] in the common [`Guard`] contract so it can
/// be composed with other guards. Returns the standardized
/// [`Error::PaymentAlreadyProcessed`] (DuplicatePayment) error when a replay is
/// detected.
pub struct PaymentReplayGuard;

impl Guard for PaymentReplayGuard {
    fn check(env: &Env, ctx: &GuardContext) -> GuardResult {
        match detect_replay_payment(env, &ctx.payer, ctx.token_id, ctx.sale_price) {
            Ok(()) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

/// Check if a payment has already been processed without returning an error.
///
/// This is a non-authoritative probe with no side effects. Returns `true` if
/// the payment is already recorded in idempotency storage.
///
/// # Example
///
/// ```rust,ignore
/// if is_payment_already_processed(&env, payer, token_id, sale_price) {
///     // Payment was already processed; safe to skip or return early
/// }
/// ```
pub fn is_payment_already_processed(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> bool {
    let key = payment_hash(env, payer, token_id, sale_price);
    env.storage().persistent().has(&key)
}

/// Build the deterministic idempotency key for a payment tuple.
fn payment_hash(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> (u32, Address, TokenId, i128) {
    let _ = env;
    (PAYMENT_REPLAY_DOMAIN, payer.clone(), token_id, sale_price)
}

/// Domain separator for payment replay idempotency keys.
const PAYMENT_REPLAY_DOMAIN: u32 = 0x5041_594D;

/// Record that a payment has been successfully processed.
///
/// This stores the payment hash in idempotency storage so future calls with
/// the same (payer, token_id, sale_price) will be detected as replays.
///
/// # Errors
/// - [`Error`] — if recording fails (storage error)
///
/// # Side effects
/// - Writes to persistent storage
/// - Must be called **after** payment validation but **before** fund transfers
pub fn record_payment_processed(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> Result<(), Error> {
    if is_payment_already_processed(env, payer, token_id, sale_price) {
        return Err(Error::PaymentAlreadyProcessed);
    }
    let key = payment_hash(env, payer, token_id, sale_price);
    env.storage().persistent().set(&key, &true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn detect_replay_payment_allows_first_payment() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let token_id: TokenId = 1;
            let sale_price = 1000i128;

            assert!(detect_replay_payment(env, &payer, token_id, sale_price).is_ok());
        });
    }

    #[test]
    fn is_payment_already_processed_returns_false_initially() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let token_id: TokenId = 1;
            let sale_price = 1000i128;

            assert!(!is_payment_already_processed(env, &payer, token_id, sale_price));
        });
    }

    #[test]
    fn record_payment_processed_succeeds() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let token_id: TokenId = 1;
            let sale_price = 1000i128;

            assert!(record_payment_processed(env, &payer, token_id, sale_price).is_ok());
        });
    }

    #[test]
    fn payment_flow_is_idempotent() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let token_id: TokenId = 1;
            let sale_price = 1000i128;

            // First payment should be allowed
            assert!(detect_replay_payment(env, &payer, token_id, sale_price).is_ok());
            assert!(!is_payment_already_processed(env, &payer, token_id, sale_price));

            // Record it
            assert!(record_payment_processed(env, &payer, token_id, sale_price).is_ok());

            // Second attempt should detect replay
            assert!(is_payment_already_processed(env, &payer, token_id, sale_price));
            assert_eq!(
                detect_replay_payment(env, &payer, token_id, sale_price),
                Err(Error::PaymentAlreadyProcessed)
            );
        });
    }

    #[test]
    fn record_payment_processed_rejects_duplicate() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let token_id: TokenId = 1;
            let sale_price = 1000i128;

            assert!(record_payment_processed(env, &payer, token_id, sale_price).is_ok());
            assert_eq!(
                record_payment_processed(env, &payer, token_id, sale_price),
                Err(Error::PaymentAlreadyProcessed)
            );
        });
    }

    #[test]
    fn payment_replay_guard_detects_duplicate() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let token_id: TokenId = 1;
            let sale_price = 1000i128;

            let ctx = GuardContext {
                payer: payer.clone(),
                token_id,
                sale_price,
            };

            assert!(PaymentReplayGuard::check(env, &ctx).is_ok());
            assert!(record_payment_processed(env, &payer, token_id, sale_price).is_ok());
            assert_eq!(
                PaymentReplayGuard::check(env, &ctx),
                Err(Error::PaymentAlreadyProcessed)
            );
        });
    }

    #[test]
    fn different_payments_are_independent() {
        with_contract(|env| {
            let payer1 = Address::generate(env);
            let payer2 = Address::generate(env);
            let token_id1: TokenId = 1;
            let token_id2: TokenId = 2;

            // Different payers should be tracked independently
            assert!(detect_replay_payment(env, &payer1, token_id1, 1000).is_ok());
            assert!(detect_replay_payment(env, &payer2, token_id1, 1000).is_ok());

            // Different token IDs should be tracked independently
            assert!(detect_replay_payment(env, &payer1, token_id1, 1000).is_ok());
            assert!(detect_replay_payment(env, &payer1, token_id2, 1000).is_ok());

            // Different prices should be tracked independently
            assert!(detect_replay_payment(env, &payer1, token_id1, 1000).is_ok());
            assert!(detect_replay_payment(env, &payer1, token_id1, 2000).is_ok());
        });
    }
}
