//! Royalty payment replay protection storage.
//!
//! Persists consumed payment IDs to prevent duplicate processing of the same royalty payment.
//!
//! # Storage
//! Key: `DataKey::UsedPayment(payment_id)` → `bool` (persistent)

use soroban_sdk::{BytesN, Env};

use crate::types::{DataKey, Error};

/// Return `true` if `payment_id` has already been processed.
pub fn is_payment_processed(env: &Env, payment_id: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::UsedPayment(payment_id.clone()))
}

/// Persist a payment ID after a successful royalty payment.
///
/// Returns the standardized duplicate payment error (`Error::PaymentAlreadyProcessed`)
/// if the payment ID has already been recorded, preventing replay.
pub fn mark_payment_processed(env: &Env, payment_id: &BytesN<32>) -> Result<(), Error> {
    if is_payment_processed(env, payment_id) {
        return Err(Error::PaymentAlreadyProcessed);
    }
    env.storage()
        .persistent()
        .set(&DataKey::UsedPayment(payment_id.clone()), &true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::BytesN as _;

    #[test]
    fn stores_payment_id_and_detects_duplicate() {
        let env = Env::default();
        let payment_id = BytesN::<32>::random(&env);

        assert!(!is_payment_processed(&env, &payment_id));
        mark_payment_processed(&env, &payment_id).unwrap();

        assert!(is_payment_processed(&env, &payment_id));
        assert_eq!(
            mark_payment_processed(&env, &payment_id),
            Err(Error::PaymentAlreadyProcessed)
        );
    }

    #[test]
    fn duplicate_payment_returns_standardized_error() {
        let env = Env::default();
        let payment_id = BytesN::<32>::random(&env);

        mark_payment_processed(&env, &payment_id).unwrap();

        let result = mark_payment_processed(&env, &payment_id);
        assert_eq!(result, Err(Error::PaymentAlreadyProcessed));
    }

    #[test]
    fn prevents_replay_of_same_payment() {
        let env = Env::default();
        let payment_id = BytesN::<32>::random(&env);

        // First processing succeeds.
        assert!(mark_payment_processed(&env, &payment_id).is_ok());

        // Any subsequent attempt to replay the same payment is rejected.
        for _ in 0..3 {
            assert_eq!(
                mark_payment_processed(&env, &payment_id),
                Err(Error::PaymentAlreadyProcessed)
            );
        }

        assert!(is_payment_processed(&env, &payment_id));
    }

    #[test]
    fn distinct_payments_are_independent() {
        let env = Env::default();
        let first = BytesN::<32>::random(&env);
        let second = BytesN::<32>::random(&env);

        assert!(mark_payment_processed(&env, &first).is_ok());
        assert!(mark_payment_processed(&env, &second).is_ok());

        assert!(is_payment_processed(&env, &first));
        assert!(is_payment_processed(&env, &second));

        assert_eq!(
            mark_payment_processed(&env, &first),
            Err(Error::PaymentAlreadyProcessed)
        );
        assert_eq!(
            mark_payment_processed(&env, &second),
            Err(Error::PaymentAlreadyProcessed)
        );
    }
}