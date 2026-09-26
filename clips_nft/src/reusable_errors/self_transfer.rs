//! Reusable self-transfer guard (issue #990).
//!
//! Provides the standardized self-transfer error together with helpers used
//! before a transfer or marketplace operation moves an NFT, so a token can
//! never be "transferred" to the address that already owns it.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::reusable_errors::self_transfer;
//!
//! self_transfer::ensure_no_self_transfer(&from, &to)?; // Err(SelfTransferNotAllowed)
//! ```

use soroban_sdk::{contracterror, Address};

/// Standardized self-transfer error for the ClipCash contract.
///
/// Carries code `241` from the `transfer` module block and matches the
/// centralized error registry.
#[allow(clippy::enum_variant_names)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SelfTransferNotAllowedError {
    /// The sender attempted to transfer a token to itself.
    SelfTransferNotAllowed = 241,
}

impl SelfTransferNotAllowedError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "transfer";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        SelfTransferNotAllowedError::SelfTransferNotAllowed as u32
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        "SelfTransferNotAllowed"
    }

    /// Decode a [`SelfTransferNotAllowedError`] from a numeric code.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            241 => Some(SelfTransferNotAllowedError::SelfTransferNotAllowed),
            _ => None,
        }
    }
}

/// Return true when the recipient is the same address as the sender.
pub fn is_self_transfer(from: &Address, to: &Address) -> bool {
    from == to
}

/// Return the standardized error when the sender and recipient are the same.
///
/// Used before a transfer or marketplace operation moves a token between the
/// same address.
pub fn ensure_no_self_transfer(
    from: &Address,
    to: &Address,
) -> Result<(), SelfTransferNotAllowedError> {
    match is_self_transfer(from, to) {
        false => Ok(()),
        true => Err(SelfTransferNotAllowedError::SelfTransferNotAllowed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn code_and_name_match_registry() {
        assert_eq!(
            SelfTransferNotAllowedError::SelfTransferNotAllowed.code(),
            241
        );
        assert_eq!(
            SelfTransferNotAllowedError::SelfTransferNotAllowed.name(),
            "SelfTransferNotAllowed"
        );
        assert_eq!(SelfTransferNotAllowedError::MODULE, "transfer");
        assert_eq!(
            SelfTransferNotAllowedError::from_code(241),
            Some(SelfTransferNotAllowedError::SelfTransferNotAllowed)
        );
        assert_eq!(SelfTransferNotAllowedError::from_code(242), None);
    }

    #[test]
    fn error_is_copy() {
        let a = SelfTransferNotAllowedError::SelfTransferNotAllowed;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_addresses_are_allowed() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        assert!(!is_self_transfer(&from, &to));
        assert!(ensure_no_self_transfer(&from, &to).is_ok());
    }

    #[test]
    fn transfer_to_same_address_is_rejected() {
        let env = Env::default();
        let holder = Address::generate(&env);
        assert!(is_self_transfer(&holder, &holder));
        assert_eq!(
            ensure_no_self_transfer(&holder, &holder),
            Err(SelfTransferNotAllowedError::SelfTransferNotAllowed)
        );
    }

    #[test]
    fn reused_during_marketplace_operations() {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            // A marketplace buyout transferring to a fresh wallet is fine...
            let buyer = Address::generate(&env);
            let escrow = Address::generate(&env);
            assert!(ensure_no_self_transfer(&buyer, &escrow).is_ok());

            // ...but accepting your own offer is a no-op that must error.
            assert_eq!(
                ensure_no_self_transfer(&buyer, &buyer),
                Err(SelfTransferNotAllowedError::SelfTransferNotAllowed)
            );
        });
    }
}
