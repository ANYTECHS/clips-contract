//! Reusable frozen-token guard (issue #991).
//!
//! Provides the standardized frozen-token error together with helpers used
//! before a transfer or marketplace operation moves an NFT, so a frozen
//! token can never be transferred, listed or bought.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::reusable_errors::frozen_token;
//!
//! frozen_token::require_not_frozen(env, token_id)?; // Err(FrozenToken)
//! ```

use crate::types::TokenId;
use soroban_sdk::{contracterror, Env};

/// Standardized frozen-token error for the ClipCash contract.
///
/// Carries code `242` from the `transfer` module block and matches the
/// centralized error registry.
#[allow(clippy::enum_variant_names)]
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FrozenTokenError {
    /// The token is frozen and cannot be moved or modified.
    FrozenToken = 242,
}

impl FrozenTokenError {
    /// Name of the module that owns this error type in the error registry.
    pub const MODULE: &'static str = "transfer";

    /// Return the unique numeric code assigned to this error.
    pub const fn code(self) -> u32 {
        FrozenTokenError::FrozenToken as u32
    }

    /// Return the machine-readable name of this error.
    pub const fn name(self) -> &'static str {
        "FrozenToken"
    }

    /// Decode a [`FrozenTokenError`] from a numeric code.
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            242 => Some(FrozenTokenError::FrozenToken),
            _ => None,
        }
    }
}

/// Return true when the given token is currently frozen.
///
/// Delegates to the shared [`crate::frozen_token::is_frozen`] flag so every
/// guard reads the same frozen-token state.
pub fn is_token_frozen(env: &Env, token_id: TokenId) -> bool {
    crate::frozen_token::is_frozen(env, token_id)
}

/// Return the standardized error when the token is frozen.
///
/// Used before a transfer or marketplace operation moves a token.
pub fn require_not_frozen(env: &Env, token_id: TokenId) -> Result<(), FrozenTokenError> {
    match is_token_frozen(env, token_id) {
        false => Ok(()),
        true => Err(FrozenTokenError::FrozenToken),
    }
}

/// Pure guard over a pre-computed frozen state.
///
/// Useful where the freeze flag was already loaded and only the standardized
/// error output is needed.
pub fn ensure_not_frozen(is_frozen_state: bool) -> Result<(), FrozenTokenError> {
    match is_frozen_state {
        false => Ok(()),
        true => Err(FrozenTokenError::FrozenToken),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;

    #[test]
    fn code_and_name_match_registry() {
        assert_eq!(FrozenTokenError::FrozenToken.code(), 242);
        assert_eq!(FrozenTokenError::FrozenToken.name(), "FrozenToken");
        assert_eq!(FrozenTokenError::MODULE, "transfer");
        assert_eq!(
            FrozenTokenError::from_code(242),
            Some(FrozenTokenError::FrozenToken)
        );
        assert_eq!(FrozenTokenError::from_code(241), None);
    }

    #[test]
    fn error_is_copy() {
        let a = FrozenTokenError::FrozenToken;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn ensure_not_frozen_guard() {
        assert_eq!(ensure_not_frozen(false), Ok(()));
        assert_eq!(ensure_not_frozen(true), Err(FrozenTokenError::FrozenToken));
    }

    #[test]
    fn frozen_token_blocks_transfer() {
        fn with_contract<F, R>(f: F) -> R
        where
            F: FnOnce(&Env) -> R,
        {
            let env = soroban_sdk::Env::default();
            let contract_id = env.register(AtomicMintContract, ());
            env.as_contract(&contract_id, || f(&env))
        }

        with_contract(|env| {
            // A freshly minted token is not frozen -> transfer proceeds.
            assert!(require_not_frozen(env, 1).is_ok());

            // After freezing, any transfer is rejected with FrozenToken.
            crate::frozen_token::freeze_token(env, 1);
            assert_eq!(
                require_not_frozen(env, 1),
                Err(FrozenTokenError::FrozenToken)
            );
        });
    }

    #[test]
    fn frozen_token_blocks_marketplace_operations() {
        fn with_contract<F, R>(f: F) -> R
        where
            F: FnOnce(&Env) -> R,
        {
            let env = soroban_sdk::Env::default();
            let contract_id = env.register(AtomicMintContract, ());
            env.as_contract(&contract_id, || f(&env))
        }

        with_contract(|env| {
            // Listing a frozen token must be rejected.
            crate::frozen_token::freeze_token(env, 9);
            assert_eq!(
                require_not_frozen(env, 9),
                Err(FrozenTokenError::FrozenToken)
            );
        });
    }
}
