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
}
