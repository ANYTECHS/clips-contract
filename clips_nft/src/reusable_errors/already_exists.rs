//! Reusable token-already-exists guard (issue #992).
//!
//! Provides the standardized token-already-exists error together with helpers
//! used during minting (and contract metadata/batch operations) so a token ID
//! can never be minted twice.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::reusable_errors::already_exists;
//!
//! already_exists::ensure_unique_token(env, token_id)?; // Err(TokenAlreadyExists)
//! ```

use crate::token_owner_storage;
use crate::types::TokenId;
use soroban_sdk::{contracterror, Env};

/// Standardized token-already-exists error for the ClipCash contract.
///
/// Carries code `250` from the `minting` module block and matches the
/// centralized error registry.
#[allow(clippy::enum_variant_names)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TokenAlreadyExistsError {
    /// The token ID is already in use by an existing NFT.
    TokenAlreadyExists = 250,
}

impl TokenAlreadyExistsError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "minting";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        TokenAlreadyExistsError::TokenAlreadyExists as u32
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        "TokenAlreadyExists"
    }

    /// Decode a [`TokenAlreadyExistsError`] from a numeric code.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            250 => Some(TokenAlreadyExistsError::TokenAlreadyExists),
            _ => None,
        }
    }
}

/// Return the standardized error when a token already exists.
///
/// Pure guard used during minting once duplicate existence is known.
pub fn ensure_token_does_not_exist(exists: bool) -> Result<(), TokenAlreadyExistsError> {
    match exists {
        false => Ok(()),
        true => Err(TokenAlreadyExistsError::TokenAlreadyExists),
    }
}

/// Return the standardized error when the token ID is already owned.
///
/// Uses the shared token-owner storage so the mint, metadata and batch flows
/// all reject duplicate IDs.
pub fn ensure_unique_token(env: &Env, token_id: TokenId) -> Result<(), TokenAlreadyExistsError> {
    ensure_token_does_not_exist(token_owner_storage::has_owner(env, token_id))
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
    fn code_and_name_match_registry() {
        assert_eq!(TokenAlreadyExistsError::TokenAlreadyExists.code(), 250);
        assert_eq!(
            TokenAlreadyExistsError::TokenAlreadyExists.name(),
            "TokenAlreadyExists"
        );
        assert_eq!(TokenAlreadyExistsError::MODULE, "minting");
        assert_eq!(
            TokenAlreadyExistsError::from_code(250),
            Some(TokenAlreadyExistsError::TokenAlreadyExists)
        );
        assert_eq!(TokenAlreadyExistsError::from_code(251), None);
    }

    #[test]
    fn error_is_copy() {
        let a = TokenAlreadyExistsError::TokenAlreadyExists;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn ensure_token_does_not_exist_guard() {
        assert!(ensure_token_does_not_exist(false).is_ok());
        assert_eq!(
            ensure_token_does_not_exist(true),
            Err(TokenAlreadyExistsError::TokenAlreadyExists)
        );
    }

    #[test]
    fn duplicate_mint_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);

            // First mint of the token ID succeeds.
            assert!(ensure_unique_token(env, 5).is_ok());
            token_owner_storage::assign_owner(env, 5, &owner, 1).unwrap();

            // Minting the same token ID again must be rejected.
            assert_eq!(
                ensure_unique_token(env, 5),
                Err(TokenAlreadyExistsError::TokenAlreadyExists)
            );
        });
    }

    #[test]
    fn reused_during_metadata_updates() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::assign_owner(env, 2, &owner, 1).unwrap();

            // Metadata updates on an existing token reuse the same guard to
            // confirm the target exists, not a fresh mint.
            assert!(token_owner_storage::has_owner(env, 2));
            assert_eq!(
                ensure_unique_token(env, 2),
                Err(TokenAlreadyExistsError::TokenAlreadyExists)
            );
        });
    }
}
