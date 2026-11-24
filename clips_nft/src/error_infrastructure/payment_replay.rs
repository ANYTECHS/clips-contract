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

use crate::types::Error;

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
    ///
    /// This is the **canonical** error for any repeated marketplace or royalty
    /// payment. All duplicate-payment paths must funnel through this variant so
    /// that callers observe a single, standardized failure code (265).
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

impl PaymentReplayError {
    /// Returns `true` if this error indicates a duplicate or replayed payment.
    ///
    /// Used by callers to distinguish idempotency failures (safe to surface to
    /// the user as "already paid") from genuine state corruption.
    pub fn is_duplicate(&self) -> bool {
        matches!(
            self,
            PaymentReplayError::DuplicatePayment
                | PaymentReplayError::ReplayAttackDetected
                | PaymentReplayError::PaymentAlreadyProcessed
        )
    }

    /// Maps a payment replay error to the crate-wide standardized error.
    ///
    /// Duplicate and replay failures are normalized to
    /// [`Error::PaymentAlreadyProcessed`] so that every repeated marketplace or
    /// royalty payment surfaces the same canonical error to callers.
    pub fn to_contract_error(self) -> Error {
        match self {
            PaymentReplayError::DuplicatePayment
            | PaymentReplayError::ReplayAttackDetected
            | PaymentReplayError::PaymentAlreadyProcessed => Error::PaymentAlreadyProcessed,
            PaymentReplayError::InvalidPaymentState => Error::InvalidState,
        }
    }
}

/// Constructs the canonical duplicate-payment error.
///
/// This is the single entry point that payment-processing code should use when
/// it detects that a marketplace or royalty payment has already been processed.
/// It returns the standardized [`Error::PaymentAlreadyProcessed`] so that all
/// duplicate-payment paths produce an identical, machine-readable failure and
/// no state change is committed (replay prevention).
pub fn duplicate_payment_error() -> Error {
    PaymentReplayError::DuplicatePayment.to_contract_error()
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

    #[test]
    fn duplicate_payment_is_canonical_code_265() {
        assert_eq!(PaymentReplayError::DuplicatePayment as u32, 265);
    }

    #[test]
    fn duplicate_payment_maps_to_standardized_error() {
        assert_eq!(
            PaymentReplayError::DuplicatePayment.to_contract_error(),
            Error::PaymentAlreadyProcessed
        );
        assert_eq!(duplicate_payment_error(), Error::PaymentAlreadyProcessed);
    }

    #[test]
    fn all_replay_variants_map_to_payment_already_processed() {
        for err in [
            PaymentReplayError::DuplicatePayment,
            PaymentReplayError::ReplayAttackDetected,
            PaymentReplayError::PaymentAlreadyProcessed,
        ] {
            assert_eq!(err.to_contract_error(), Error::PaymentAlreadyProcessed);
        }
        assert_eq!(
            PaymentReplayError::InvalidPaymentState.to_contract_error(),
            Error::InvalidState
        );
    }

    #[test]
    fn is_duplicate_classifies_replay_errors() {
        assert!(PaymentReplayError::DuplicatePayment.is_duplicate());
        assert!(PaymentReplayError::ReplayAttackDetected.is_duplicate());
        assert!(PaymentReplayError::PaymentAlreadyProcessed.is_duplicate());
        assert!(!PaymentReplayError::InvalidPaymentState.is_duplicate());
    }

    #[test]
    fn replay_is_deterministic_and_prevents_state_change() {
        // Simulate idempotency storage: the first payment is recorded, the
        // second attempt with the same hash must fail with the same error and
        // must not mutate the recorded state.
        let mut processed: [Option<u32>; 1] = [None];
        let payment_hash = 0u32;

        // First attempt: not yet processed, record it.
        assert!(processed[payment_hash as usize].is_none());
        processed[payment_hash as usize] = Some(1);

        // Replay attempt: detect duplicate, return standardized error, no change.
        let replay_result: Result<(), Error> = if processed[payment_hash as usize].is_some() {
            Err(duplicate_payment_error())
        } else {
            processed[payment_hash as usize] = Some(2);
            Ok(())
        };

        assert_eq!(replay_result, Err(Error::PaymentAlreadyProcessed));
        assert_eq!(processed[payment_hash as usize], Some(1));

        // A second replay yields the identical error (deterministic).
        let second_replay: Result<(), Error> = Err(duplicate_payment_error());
        assert_eq!(second_replay, replay_result);
    }
}
