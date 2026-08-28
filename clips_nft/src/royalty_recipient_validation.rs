//! Royalty recipient validation (issue #789).
//!
//! Validates the wallet that will receive royalty payments **before** any
//! royalty configuration is accepted. Ensures the supplied address is a
//! well-formed Stellar address and that it is a valid recipient (i.e. not the
//! contract's own address, which cannot usefully receive royalty payments).
//!
//! # Errors
//! - [`Error::InvalidAddress`] — the address string is not a valid Stellar
//!   address (malformed / unparseable).
//! - [`Error::InvalidRecipient`] — the address is structurally valid but is not
//!   an acceptable royalty recipient (e.g. the contract itself).
//!
//! # Acceptance criteria
//! - **Validate Stellar address** — [`validate_royalty_recipient_address`]
//! - **Reject invalid recipient** — rejects the contract's own address
//! - **Return descriptive error** — distinct errors for malformed vs. rejected
//! - **Add tests** — unit tests in this module

use soroban_sdk::{Address, Env, String};

use crate::types::Error;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Validate a Stellar address string and, when present, the resolved recipient
/// address.
///
/// `encoded` must parse into a valid Stellar [`Address`] and re-encode to the
/// same canonical string; otherwise [`Error::InvalidAddress`] is returned.
/// When `recipient` is provided it must not be the contract's own address,
/// otherwise [`Error::InvalidRecipient`] is returned.
pub fn validate_royalty_recipient_address(
    env: &Env,
    encoded: &String,
    recipient: Option<&Address>,
) -> Result<(), Error> {
    let parsed = Address::from_string(env, encoded.clone()).map_err(|_| Error::InvalidAddress)?;

    // Canonical-form check: a valid address must re-encode to the same string.
    if parsed.to_string(env) != *encoded {
        return Err(Error::InvalidAddress);
    }

    if let Some(actual) = recipient {
        let contract = env.current_contract_address();
        if *actual == contract {
            return Err(Error::InvalidRecipient);
        }
        if *actual != parsed {
            return Err(Error::InvalidAddress);
        }
    }

    Ok(())
}

/// Convenience wrapper that validates an already-materialised [`Address`]
/// recipient against the royalty-recipient business rules.
///
/// Rejects the contract's own address.
///
/// # Errors
/// Returns [`Error::InvalidRecipient`] when `recipient` equals the contract.
pub fn validate_royalty_recipient(env: &Env, recipient: &Address) -> Result<(), Error> {
    let contract = env.current_contract_address();
    if *recipient == contract {
        return Err(Error::InvalidRecipient);
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

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
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn accepts_valid_wallet_address() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let encoded = recipient.to_string(env);
            assert!(validate_royalty_recipient_address(env, &encoded, Some(&recipient)).is_ok());
        });
    }

    #[test]
    fn rejects_malformed_address_string() {
        with_contract(|env| {
            let bad = String::from_str(env, "not-a-valid-stellar-address");
            assert_eq!(
                validate_royalty_recipient_address(env, &bad, None),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn rejects_contract_self_address() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(validate_royalty_recipient(env, &contract), Err(Error::InvalidRecipient));

            let encoded = contract.to_string(env);
            assert_eq!(
                validate_royalty_recipient_address(env, &encoded, Some(&contract)),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn rejects_recipient_mismatch_with_encoded_address() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let encoded = a.to_string(env);
            assert_eq!(
                validate_royalty_recipient_address(env, &encoded, Some(&b)),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn accepts_distinct_generated_addresses() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            assert!(validate_royalty_recipient(env, &a).is_ok());
            assert!(validate_royalty_recipient(env, &b).is_ok());
            assert_ne!(a, b);
        });
    }

    #[test]
    fn encoded_round_trip_rejects_empty_string() {
        with_contract(|env| {
            // The empty string is not a valid Stellar address encoding.
            let empty = String::from_str(env, "");
            assert_eq!(
                validate_royalty_recipient_address(env, &empty, None),
                Err(Error::InvalidAddress)
            );
        });
    }
}
