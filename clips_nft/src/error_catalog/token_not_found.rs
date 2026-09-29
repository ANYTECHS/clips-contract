//! Standardized "token not found" error (issue #987).
//!
//! Returned whenever an operation references an NFT that does not exist.
//! The error carries a unique code (`230`) and is intended to be reused
//! across every NFT-facing module: minting, transfers, royalties and the
//! marketplace.

use soroban_sdk::{contracterror, Env};

use crate::types::TokenId;

/// Error returned when an operation references an NFT that does not exist.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TokenNotFoundError {
    /// The referenced NFT does not exist.
    TokenNotFound = 230,
}

impl TokenNotFoundError {
    /// Unique numeric code assigned in the centralized error registry.
    pub const CODE: u32 = 230;

    /// Machine-readable error name.
    pub const NAME: &'static str = "TokenNotFound";

    /// Module that owns this error in the centralized error registry.
    pub const MODULE: &'static str = "core";

    /// Return the unique numeric code for this error.
    pub const fn code(self) -> u32 {
        Self::CODE
    }

    /// Return the machine-readable name for this error.
    pub const fn name(self) -> &'static str {
        Self::NAME
    }
}

/// Ensure that an NFT with `token_id` exists in contract storage.
///
/// Returns [`TokenNotFoundError`] when no ownership record exists. This is
/// the reusable check used across NFT modules before any read or write.
pub fn ensure_token_exists(env: &Env, token_id: TokenId) -> Result<(), TokenNotFoundError> {
    require_token_exists(crate::token_owner_storage::has_owner(env, token_id))
}

/// Pure guard: return `Ok(())` when `exists` is `true`, otherwise
/// [`TokenNotFoundError`]. Useful for callers that already know existence.
pub fn require_token_exists(exists: bool) -> Result<(), TokenNotFoundError> {
    if exists {
        Ok(())
    } else {
        Err(TokenNotFoundError::TokenNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_owner_storage;
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
        assert_eq!(TokenNotFoundError::TokenNotFound.code(), 230);
        assert_eq!(TokenNotFoundError::TokenNotFound.name(), "TokenNotFound");
        assert_eq!(TokenNotFoundError::MODULE, "core");
    }

    #[test]
    fn error_is_copy() {
        let a = TokenNotFoundError::TokenNotFound;
        let b = a; // Copy, not a move
        assert_eq!(a, b);
    }

    #[test]
    fn require_token_exists_rejects_missing_tokens() {
        assert_eq!(
            require_token_exists(false),
            Err(TokenNotFoundError::TokenNotFound)
        );
        assert!(require_token_exists(true).is_ok());
    }

    #[test]
    fn ensure_token_exists_uses_storage_across_nft_modules() {
        with_contract(|env| {
            let owner = Address::generate(env);

            // No NFT exists yet -> TokenNotFound (e.g. transfer/marketplace read).
            assert_eq!(
                ensure_token_exists(env, 7),
                Err(TokenNotFoundError::TokenNotFound)
            );

            // After minting, the NFT exists -> reuse guard reports OK.
            token_owner_storage::assign_owner(env, 7, &owner, 100).unwrap();
            assert!(ensure_token_exists(env, 7).is_ok());
        });
    }
}
