//! Metadata update guard.
//!
//! Validates authorization and NFT state before allowing metadata updates.
//!
//! # Guards
//! 1. Verifies token existence (implied by getting the owner).
//! 2. Verifies the token is not frozen.
//! 3. Verifies the caller is the owner or the contract admin.
//! 4. Verifies the metadata update policy (e.g. one-time update).

use soroban_sdk::{Address, Env};

use crate::frozen_token;
use crate::metadata_update_policy;
use crate::token_owner_storage;
use crate::types::{DataKey, Error, TokenId};

/// Validates all preconditions before an NFT's metadata can be updated.
///
/// # Errors
/// - [`Error::TokenNotFound`] if the token does not exist.
/// - [`Error::Unauthorized`] if the token is frozen, or the caller is not the owner/admin.
/// - [`Error::MetadataAlreadyUpdated`] if the update policy rejects the change.
pub fn check_metadata_update(env: &Env, token_id: TokenId, caller: &Address) -> Result<(), Error> {
    caller.require_auth();

    // 1. Verify token exists by fetching its owner.
    let owner = token_owner_storage::get_owner(env, token_id)?;

    // 2. Verify token state — frozen tokens cannot have their metadata modified.
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }

    // 3. Verify authorized caller (owner or admin).
    let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
    let is_admin = if let Some(ref a) = admin {
        caller == a
    } else {
        false
    };

    if !is_admin && caller != &owner {
        return Err(Error::Unauthorized);
    }

    // 4. Enforce metadata update policy (e.g., one-time update).
    metadata_update_policy::check_update_allowed(env, token_id, caller)?;

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
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    fn setup_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    #[test]
    fn owner_can_update_metadata() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_metadata_update(env, 1, &owner).is_ok());
        });
    }

    #[test]
    fn admin_can_update_metadata() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token(env, 1, &owner);
            assert!(check_metadata_update(env, 1, &admin).is_ok());
        });
    }

    #[test]
    fn unauthorized_caller_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                check_metadata_update(env, 1, &stranger),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn nonexistent_token_rejected() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                check_metadata_update(env, 1, &caller),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn frozen_token_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            assert_eq!(
                check_metadata_update(env, 1, &owner),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn update_policy_enforced() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            
            // First update is OK
            assert!(check_metadata_update(env, 1, &owner).is_ok());
            
            // Mark as used
            metadata_update_policy::mark_update_used(env, 1);
            
            // Second update by owner fails
            assert_eq!(
                check_metadata_update(env, 1, &owner),
                Err(Error::MetadataAlreadyUpdated)
            );
        });
    }
}
