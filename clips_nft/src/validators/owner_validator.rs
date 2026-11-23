//! Owner Address Validator (Issue #1091).
//!
//! Validates that a supplied owner address matches the stored NFT owner.

use soroban_sdk::{Address, Env};

use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// Validate that `supplied_owner` matches the stored owner of `token_id`.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if no owner is stored for `token_id`.
/// Returns [`Error::Unauthorized`] if `supplied_owner` does not match the stored owner.
pub fn validate_owner_address(
    env: &Env,
    token_id: TokenId,
    supplied_owner: &Address,
) -> Result<(), Error> {
    let stored_owner = token_owner_storage::get_owner(env, token_id)?;
    if supplied_owner != &stored_owner {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_validate_owner_address_success() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let token_id = 1;
        token_owner_storage::set_owner(&env, token_id, &owner);

        assert!(validate_owner_address(&env, token_id, &owner).is_ok());
    }

    #[test]
    fn test_validate_owner_address_mismatch() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let stranger = Address::generate(&env);
        let token_id = 1;
        token_owner_storage::set_owner(&env, token_id, &owner);

        assert_eq!(
            validate_owner_address(&env, token_id, &stranger),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn test_validate_owner_address_token_not_found() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let token_id = 999;

        assert_eq!(
            validate_owner_address(&env, token_id, &owner),
            Err(Error::TokenNotFound)
        );
    }
}
