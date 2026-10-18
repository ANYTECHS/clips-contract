//! Reusable invalid-recipient guard (issue #989).
//!
//! Provides the standardized invalid-recipient error together with helpers
//! used before any transfer or marketplace payout so an NFT can never be
//! sent to an unusable recipient.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::reusable_errors::invalid_recipient;
//!
//! invalid_recipient::ensure_valid_recipient(env, &recipient)?; // Err(InvalidRecipient)
//! ```

use soroban_sdk::{contracterror, Address, Env};

/// Standardized invalid-recipient error for the ClipCash contract.
///
/// Carries code `240` from the `transfer` module block and matches the
/// centralized error registry.
#[allow(clippy::enum_variant_names)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvalidRecipientError {
    /// The provided recipient address cannot receive a transfer.
    InvalidRecipient = 240,
}

impl InvalidRecipientError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "transfer";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        InvalidRecipientError::InvalidRecipient as u32
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        "InvalidRecipient"
    }

    /// Decode an [`InvalidRecipientError`] from a numeric code.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            240 => Some(InvalidRecipientError::InvalidRecipient),
            _ => None,
        }
    }
}

/// Return true when the given address is a usable recipient.
///
/// An address is unusable when it is the contract itself, i.e. a transfer
/// that would leave the NFT stuck on the contract.
pub fn is_valid_recipient(env: &Env, recipient: &Address) -> bool {
    *recipient != env.current_contract_address()
}

/// Return the standardized error when the guard is not satisfied.
///
/// Rejects transfers to the contract itself (e.g. transfers or marketplace
/// payouts that would strand the NFT on the contract).
pub fn ensure_valid_recipient(env: &Env, recipient: &Address) -> Result<(), InvalidRecipientError> {
    match is_valid_recipient(env, recipient) {
        true => Ok(()),
        false => Err(InvalidRecipientError::InvalidRecipient),
    }
}

/// Pure guard over a pre-computed validity check.
///
/// Useful where validity is already known (e.g. the caller validated the
/// address earlier) and only the standardized error output is needed.
pub fn ensure_recipient(is_valid: bool) -> Result<(), InvalidRecipientError> {
    match is_valid {
        true => Ok(()),
        false => Err(InvalidRecipientError::InvalidRecipient),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_and_name_match_registry() {
        assert_eq!(InvalidRecipientError::InvalidRecipient.code(), 240);
        assert_eq!(
            InvalidRecipientError::InvalidRecipient.name(),
            "InvalidRecipient"
        );
        assert_eq!(InvalidRecipientError::MODULE, "transfer");
        assert_eq!(
            InvalidRecipientError::from_code(240),
            Some(InvalidRecipientError::InvalidRecipient)
        );
        assert_eq!(InvalidRecipientError::from_code(239), None);
    }

    #[test]
    fn error_is_copy() {
        let a = InvalidRecipientError::InvalidRecipient;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn ensure_recipient_guard() {
        assert_eq!(
            ensure_recipient(false),
            Err(InvalidRecipientError::InvalidRecipient)
        );
        assert!(ensure_recipient(true).is_ok());
    }

    #[test]
    fn rejects_contract_itself_as_recipient() {
        use crate::AtomicMintContract;

        fn with_contract<F, R>(f: F) -> R
        where
            F: FnOnce(&Env) -> R,
        {
            let env = Env::default();
            let contract_id = env.register(AtomicMintContract, ());
            env.as_contract(&contract_id, || f(&env))
        }

        with_contract(|env| {
            let recipient = env.current_contract_address();
            assert!(!is_valid_recipient(env, &recipient));
            assert_eq!(
                ensure_valid_recipient(env, &recipient),
                Err(InvalidRecipientError::InvalidRecipient)
            );
        });
    }
}
