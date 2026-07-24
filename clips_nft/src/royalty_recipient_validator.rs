//! Royalty recipient validation (issue #671).
//!
//! Ensures the royalty recipient is a valid Stellar wallet address before any
//! royalty information is persisted. Rejects the contract's own address (which
//! cannot usefully receive royalty payments) and returns a dedicated custom
//! error so callers can distinguish recipient failures from other mint errors.
//!
//! # Errors
//! [`Error::InvalidRecipient`] when the address is not a valid royalty recipient.

use soroban_sdk::{Address, Env};

use crate::types::Error;

/// Validate that `recipient` is an acceptable royalty wallet address.
///
/// # Checks
/// - Rejects the current contract address (cannot hold / receive royalties).
///
/// # Errors
/// Returns [`Error::InvalidRecipient`] when the address fails validation.
pub fn validate_royalty_recipient(env: &Env, recipient: &Address) -> Result<(), Error> {
    let contract = env.current_contract_address();
    if *recipient == contract {
        return Err(Error::InvalidRecipient);
    }
    Ok(())
}

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
            assert!(validate_royalty_recipient(env, &recipient).is_ok());
        });
    }

    #[test]
    fn rejects_contract_self_address() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(
                validate_royalty_recipient(env, &contract),
                Err(Error::InvalidRecipient)
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
}
