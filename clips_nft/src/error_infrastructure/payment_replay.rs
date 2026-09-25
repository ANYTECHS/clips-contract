//! Standardized payment replay and duplicate detection errors (issue #1093).
//!
//! This module defines machine-readable errors for detecting and preventing
//! duplicate or replayed payments, particularly in royalty and marketplace
//! transaction contexts where payment idempotency is critical.
//!
//! # Error code allocation
//!
//! Payment replay errors are assigned codes in the range **265–268**:
//!
//! | Code | Error | Meaning |
//! |------|-------|---------|
//! | 265 | `DuplicatePayment` | Payment has already been processed |
//! | 266 | `InvalidPaymentState` | Payment state is inconsistent or invalid |
//! | 267 | `ReplayAttackDetected` | Transaction appears to be a replay of a prior one |
//! | 268 | `PaymentAlreadyProcessed` | The specific payment transaction was previously completed |
//!
//! # Replay protection strategy
//!
//! To prevent duplicate payments, the contract uses:
//!
//! 1. **Transaction signature hashing** — each payment is uniquely identified by
//!    hashing the combination of (payer, token_id, sale_price, timestamp).
//! 2. **Idempotency storage** — the hash is checked against a persistent store
//!    before processing.
//! 3. **Deterministic error** — if a replay is detected, the same error is
//!    returned and no state change occurs.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::error_infrastructure::payment_replay::PaymentReplayError;
//!
//! if has_processed_payment(&env, &payment_hash) {
//!     return Err(PaymentReplayError::PaymentAlreadyProcessed.into());
//! }
//! record_payment(&env, &payment_hash);
//! ```

use soroban_sdk::contracterror;

/// Standardized errors for payment replay detection and duplicate handling (issue #1093).
///
/// Each error corresponds to a specific failure mode in payment processing that
/// indicates either a genuine duplicate or a malicious replay attempt.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PaymentReplayError {
    /// The payment has already been processed (code 265).
    ///
    /// Returned when the contract detects that a payment matching the current
    /// (payer, token_id, sale_price) tuple was already successfully processed
    /// within a prior transaction.
    ///
    /// This is the primary replay protection error. When returned, no state
    /// change occurs; the transaction is idempotent and safe to retry.
    DuplicatePayment = 265,

    /// Payment state is inconsistent or invalid (code 266).
    ///
    /// Returned when:
    /// - A payment record exists but is in an unexpected state
    /// - Payment metadata is corrupted or missing
    /// - Internal consistency check fails
    InvalidPaymentState = 266,

    /// A replay attack is suspected (code 267).
    ///
    /// Returned when:
    /// - Multiple payments are submitted with identical parameters in rapid succession
    /// - A prior payment was attempted with the same hash
    /// - Cumulative payment amount would overflow
    ReplayAttackDetected = 267,

    /// The payment transaction was already completed (code 268).
    ///
    /// Returned when:
    /// - The payment hash is found in the idempotency storage
    /// - The payer has already transferred funds for this token sale
    /// - The royalty was already distributed for this transaction
    PaymentAlreadyProcessed = 268,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_unique() {
        let duplicate = PaymentReplayError::DuplicatePayment as u32;
        let invalid_state = PaymentReplayError::InvalidPaymentState as u32;
        let replay_attack = PaymentReplayError::ReplayAttackDetected as u32;
        let already_processed = PaymentReplayError::PaymentAlreadyProcessed as u32;

        assert_eq!(duplicate, 265);
        assert_eq!(invalid_state, 266);
        assert_eq!(replay_attack, 267);
        assert_eq!(already_processed, 268);
    }

    #[test]
    fn error_codes_are_in_correct_range() {
        for code in 265..=268u32 {
            let error = match code {
                265 => Some(PaymentReplayError::DuplicatePayment),
                266 => Some(PaymentReplayError::InvalidPaymentState),
                267 => Some(PaymentReplayError::ReplayAttackDetected),
                268 => Some(PaymentReplayError::PaymentAlreadyProcessed),
                _ => None,
            };
            assert!(error.is_some(), "code {} should be defined", code);
        }
    }
}
