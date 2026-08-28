//! Royalty freeze option (issue #794).
//!
//! Optional, **permanent** lock that prevents any future change to an NFT's
//! royalty configuration. Once frozen, the configuration can never be
//! unfrozen or modified — the transition is strictly one-way
//! (`Active → Frozen`).
//!
//! # Storage
//! Key: `DataKey::RoyaltyFrozen(token_id)` → `bool` (persistent storage).
//!
//! # Authorization
//! Freezing is an authorized action: only the contract admin, the token
//! creator, or the token owner may freeze a token's royalty configuration
//! (see [`crate::royalty_authorization`]).

use soroban_sdk::{Address, Env};

use crate::royalty_authorization::authorize_royalty_update;
use crate::token_storage;
use crate::types::{DataKey, Error, TokenId};

/// Return `true` when the token's royalty configuration is frozen.
///
/// Defaults to `false` when the marker has never been written.
pub fn is_royalty_frozen(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .get::<_, bool>(&DataKey::RoyaltyFrozen(token_id))
        .unwrap_or(false)
}

/// Reject the operation when the token's royalty configuration is frozen.
///
/// # Errors
/// - [`Error::RoyaltyFrozen`] — royalty configuration is frozen.
pub fn require_not_frozen(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if is_royalty_frozen(env, token_id) {
        return Err(Error::RoyaltyFrozen);
    }
    Ok(())
}

/// Permanently freeze `token_id`'s royalty configuration.
///
/// The caller must be an [`authorize_royalty_update`] authorized identity
/// (admin, creator, or owner). The freeze is permanent: after this call the
/// configuration can no longer be modified.
///
/// # Errors
/// - [`Error::TokenNotFound`]        — token has no royalty configuration.
/// - [`Error::UnauthorizedConfigurationUpdate`] — caller not authorized.
/// - [`Error::RoyaltyFrozen`]        — already frozen.
pub fn freeze_royalty(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    // The token must exist and have a royalty configuration.
    token_storage::get_royalty(env, token_id)?;
    // Only authorized identities may freeze.
    authorize_royalty_update(env, caller, token_id)?;
    // Freeze is one-time and permanent.
    if is_royalty_frozen(env, token_id) {
        return Err(Error::RoyaltyFrozen);
    }
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyFrozen(token_id), &true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn authorized_creator_can_freeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 1, &admin, &creator, &owner);

            assert!(freeze_royalty(env, &creator, 1).is_ok());
            assert!(is_royalty_frozen(env, 1));
        });
    }

    #[test]
    fn owner_can_freeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 2, &admin, &creator, &owner);

            assert!(freeze_royalty(env, &owner, 2).is_ok());
            assert!(is_royalty_frozen(env, 2));
        });
    }

    #[test]
    fn admin_can_freeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 3, &admin, &creator, &owner);

            assert!(freeze_royalty(env, &admin, 3).is_ok());
        });
    }

    #[test]
    fn unauthorized_caller_cannot_freeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let interloper = Address::generate(env);
            seed_token(env, 4, &admin, &creator, &owner);

            assert_eq!(
                freeze_royalty(env, &interloper, 4),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
            assert!(!is_royalty_frozen(env, 4));
        });
    }

    #[test]
    fn missing_token_cannot_be_frozen() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);
            assert_eq!(freeze_royalty(env, &admin, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn double_freeze_is_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 5, &admin, &creator, &owner);

            assert!(freeze_royalty(env, &creator, 5).is_ok());
            // Freeze is one-time: a second attempt by any identity is rejected.
            assert_eq!(freeze_royalty(env, &owner, 5), Err(Error::RoyaltyFrozen));
        });
    }

    #[test]
    fn frozen_royalty_rejects_further_operations() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, 6, &admin, &creator, &owner);

            assert!(freeze_royalty(env, &creator, 6).is_ok());
            assert_eq!(require_not_frozen(env, 6), Err(Error::RoyaltyFrozen));
        });
    }
}
