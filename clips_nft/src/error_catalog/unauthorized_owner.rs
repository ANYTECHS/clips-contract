//! Standardized "unauthorized owner" error (issue #988).
//!
//! Returned when an operation is attempted by an account that does not own
//! the NFT. The error carries a unique code (`231`) and is used during
//! transfers and marketplace operations.

use soroban_sdk::{contracterror, Address, Env};

use crate::types::TokenId;

/// Error returned when the caller does not own the referenced NFT.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum UnauthorizedOwnerError {
    /// The caller is not the owner of the referenced NFT.
    UnauthorizedOwner = 231,
}

impl UnauthorizedOwnerError {
    /// Unique numeric code assigned in the centralized error registry.
    pub const CODE: u32 = 231;

    /// Machine-readable error name.
    pub const NAME: &'static str = "UnauthorizedOwner";

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

/// Return `true` when `caller` is the current owner of `token_id`.
///
/// A clear, reusable ownership read so transfers and marketplace flows share
/// the same definition of "owner".
pub fn is_owner(env: &Env, token_id: TokenId, caller: &Address) -> bool {
    crate::token_owner_storage::get_owner(env, token_id)
        .map(|owner| owner == *caller)
        .unwrap_or(false)
}

/// Ensure `caller` owns `token_id`.
///
/// Returns [`UnauthorizedOwnerError`] when the NFT does not exist or when
/// `caller` is not its owner. Used during transfers and marketplace
/// operations (e.g. listing, offer acceptance, cancel).
pub fn require_owner(
    env: &Env,
    token_id: TokenId,
    caller: &Address,
) -> Result<(), UnauthorizedOwnerError> {
    ensure_owner(is_owner(env, token_id, caller))
}

/// Pure guard: return `Ok(())` when `is_owner` is `true`, otherwise
/// [`UnauthorizedOwnerError`].
pub fn ensure_owner(is_owner: bool) -> Result<(), UnauthorizedOwnerError> {
    if is_owner {
        Ok(())
    } else {
        Err(UnauthorizedOwnerError::UnauthorizedOwner)
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
        assert_eq!(UnauthorizedOwnerError::UnauthorizedOwner.code(), 231);
        assert_eq!(
            UnauthorizedOwnerError::UnauthorizedOwner.name(),
            "UnauthorizedOwner"
        );
        assert_eq!(UnauthorizedOwnerError::MODULE, "core");
    }

    #[test]
    fn error_is_copy() {
        let a = UnauthorizedOwnerError::UnauthorizedOwner;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn ensure_owner_guard() {
        assert_eq!(
            ensure_owner(false),
            Err(UnauthorizedOwnerError::UnauthorizedOwner)
        );
        assert!(ensure_owner(true).is_ok());
    }

    #[test]
    fn require_owner_used_during_transfer_and_marketplace() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            token_owner_storage::assign_owner(env, 3, &owner, 200).unwrap();

            // Transfer scenario: only the owner may move the NFT.
            assert!(require_owner(env, 3, &owner).is_ok());
            assert_eq!(
                require_owner(env, 3, &stranger),
                Err(UnauthorizedOwnerError::UnauthorizedOwner)
            );

            // Marketplace scenario: only the owner may list / accept an offer.
            assert_eq!(
                require_owner(env, 3, &stranger),
                Err(UnauthorizedOwnerError::UnauthorizedOwner)
            );
            assert!(require_owner(env, 3, &owner).is_ok());
        });
    }

    #[test]
    fn require_owner_rejects_unknown_tokens() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                require_owner(env, 999, &caller),
                Err(UnauthorizedOwnerError::UnauthorizedOwner)
            );
            assert!(!is_owner(env, 999, &caller));
        });
    }
}
