//! Royalty configuration update authorization (issue #792).
//!
//! Restricts royalty configuration modifications to authorized identities.
//! A caller may update a token's royalty configuration when they are:
//!
//! 1. the contract **admin** (stored at `DataKey::Admin`),
//! 2. the token **creator** ([`crate::creator_storage`]),
//! 3. the token **owner** ([`crate::token_owner_storage`]).
//!
//! Every other caller is rejected with
//! [`Error::UnauthorizedConfigurationUpdate`]. `require_auth` is enforced for
//! the matched identity before the check succeeds.

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

/// Validate that `caller` is authorized to modify the royalty configuration of
/// `token_id`.
///
/// # Errors
/// - [`Error::UnauthorizedConfigurationUpdate`] — caller matches no identity.
pub fn authorize_royalty_update(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    if let Some(admin) = env.storage().instance().get::<_, Address>(&DataKey::Admin) {
        if *caller == admin {
            caller.require_auth();
            return Ok(());
        }
    }

    if let Ok(creator) = crate::creator_storage::get_creator(env, token_id) {
        if *caller == creator {
            caller.require_auth();
            return Ok(());
        }
    }

    if let Ok(owner) = crate::token_owner_storage::get_owner(env, token_id) {
        if *caller == owner {
            caller.require_auth();
            return Ok(());
        }
    }

    Err(Error::UnauthorizedConfigurationUpdate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_storage;
    use crate::types::{Royalty, RoyaltyRecipient};
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

    fn seed_token(
        env: &Env,
        token_id: TokenId,
        admin: &Address,
        creator: &Address,
        owner: &Address,
    ) {
        env.storage().instance().set(&DataKey::Admin, admin);
        crate::creator_storage::set_creator(env, token_id, creator);
        crate::token_owner_storage::save_owner(env, token_id, owner);
        token_storage::set_royalty(
            env,
            token_id,
            &Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: Address::generate(env),
                        basis_points: 500,
                    }
                ],
                asset_address: None,
            },
        );
    }
    #[test]
    fn creator_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 1, &admin, &creator, &owner);

            assert!(authorize_royalty_update(env, &creator, 1).is_ok());
        });
    }
    #[test]
    fn owner_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 2, &admin, &creator, &owner);

            assert!(authorize_royalty_update(env, &owner, 2).is_ok());
        });
    }
    #[test]
    fn admin_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 3, &admin, &creator, &owner);

            assert!(authorize_royalty_update(env, &admin, 3).is_ok());
        });
    }
    #[test]
    fn unauthorized_caller_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let interloper = Address::generate(env);
            seed_token(env, 4, &admin, &creator, &owner);

            assert_eq!(
                authorize_royalty_update(env, &interloper, 4),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }
    #[test]
    fn unregistered_token_rejects_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let caller = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            // Contract admin is always authorized; any other caller (with no
            // creator/owner record to match) is rejected.
            assert!(authorize_royalty_update(env, &admin, 999).is_ok());
            assert_eq!(
                authorize_royalty_update(env, &caller, 999),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    /// Verify that a random third-party address cannot modify royalty
    /// configuration even if a token and all authorized roles exist.
    #[test]
    fn third_party_cannot_modify_royalty_config() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            // Several would-be attackers that hold no authorized role.
            let attacker_1 = Address::generate(env);
            let attacker_2 = Address::generate(env);
            seed_token(env, 5, &admin, &creator, &owner);

            assert_eq!(
                authorize_royalty_update(env, &attacker_1, 5),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
            assert_eq!(
                authorize_royalty_update(env, &attacker_2, 5),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    /// Changing the recipient address is only possible for authorized callers.
    /// This test simulates an unauthorized attempt to change the royalty
    /// recipient and confirms the guard prevents the update.
    #[test]
    fn unauthorized_recipient_change_is_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let unauthorized = Address::generate(env);
            seed_token(env, 6, &admin, &creator, &owner);

            // Guard must reject the unauthorized address before any storage
            // mutation can occur.
            let result = authorize_royalty_update(env, &unauthorized, 6);
            assert_eq!(result, Err(Error::UnauthorizedConfigurationUpdate));

            // Verify the royalty configuration is still intact (unchanged)
            // by confirming an authorized caller can still perform the update.
            assert!(authorize_royalty_update(env, &admin, 6).is_ok());
        });
    }

    /// Verify that the creator retains authority to update royalty rate even
    /// after the token has been transferred to a new owner.
    #[test]
    fn creator_retains_royalty_authority_after_transfer() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let original_owner = Address::generate(env);
            let new_owner = Address::generate(env);
            seed_token(env, 7, &admin, &creator, &original_owner);
            // Simulate a transfer: update the token owner record.
            crate::token_owner_storage::save_owner(env, 7, &new_owner);

            // Creator is still authorized after ownership transfer.
            assert!(authorize_royalty_update(env, &creator, 7).is_ok());
            // New owner is also authorized.
            assert!(authorize_royalty_update(env, &new_owner, 7).is_ok());
            // Old owner is no longer authorized.
            assert_eq!(
                authorize_royalty_update(env, &original_owner, 7),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }
}
