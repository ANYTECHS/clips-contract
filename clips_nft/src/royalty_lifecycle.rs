//! Royalty lifecycle state validation (issue #795).
//!
//! A token's royalty configuration lives in exactly one of two mutually
//! exclusive states:
//!
//! - **Active** — royalty configured and mutable;
//! - **Frozen**  — royalty configured, immutable (see [`crate::royalty_freeze`]).
//!
//! The lifecycle is strictly one-way: only the `Active → Frozen` transition is
//! legal (performed by [`crate::royalty_freeze::freeze_royalty`]). A frozen
//! configuration can never be modified or unfrozen, so any attempted
//! transition away from `Frozen` is invalid and rejected.
//!
//! All royalty-modifying entry points must first validate the token's state
//! via [`validate_state_for_update`] before applying changes.

use soroban_sdk::Env;

use crate::royalty_freeze::is_royalty_frozen;
use crate::token_storage;
use crate::types::{Error, TokenId};

/// The lifecycle state of a token's royalty configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoyaltyLifecycleState {
    /// Royalty configured and mutable.
    Active,
    /// Royalty configured and permanently immutable.
    Frozen,
}

/// Resolve the current lifecycle state of `token_id`'s royalty configuration.
///
/// A token with no royalty configuration yields [`Error::TokenNotFound`]:
/// state resolution requires an existing, tracked configuration.
///
/// # Errors
/// - [`Error::TokenNotFound`] — token has no royalty configuration.
pub fn royalty_state(env: &Env, token_id: TokenId) -> Result<RoyaltyLifecycleState, Error> {
    // Existence check first: unknown tokens are rejected before any state
    // transition can be considered.
    token_storage::get_royalty(env, token_id)?;
    Ok(if is_royalty_frozen(env, token_id) {
        RoyaltyLifecycleState::Frozen
    } else {
        RoyaltyLifecycleState::Active
    })
}

/// Validate that `token_id`'s royalty configuration may be updated.
///
/// Rejects:
/// - missing configurations ([`Error::TokenNotFound`]);
/// - **invalid transitions** on frozen configurations
///   ([`Error::RoyaltyFrozen`]) — a frozen royalty is permanent and can never
///   be changed.
///
/// # Errors
/// - [`Error::TokenNotFound`] — token has no royalty configuration.
/// - [`Error::RoyaltyFrozen`] — state transition is invalid (frozen).
pub fn validate_state_for_update(env: &Env, token_id: TokenId) -> Result<(), Error> {
    match royalty_state(env, token_id)? {
        RoyaltyLifecycleState::Active => Ok(()),
        RoyaltyLifecycleState::Frozen => Err(Error::RoyaltyFrozen),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::royalty_freeze::freeze_royalty;
    use crate::types::{Royalty, RoyaltyRecipient};
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, vec, Address, Env};

    const TOKEN: TokenId = 1;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn seed(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        env.storage()
            .instance()
            .set(&crate::types::DataKey::Admin, &admin);
        crate::creator_storage::set_creator(env, TOKEN, &creator);
        crate::token_owner_storage::save_owner(env, TOKEN, &owner);
        token_storage::set_royalty(
            env,
            TOKEN,
            &Royalty {
                recipients: vec![
                    env,
                    RoyaltyRecipient {
                        recipient: Address::generate(env),
                        basis_points: 500,
                    },
                ],
                asset_address: None,
            },
        );
        (admin, creator, owner)
    }

    #[test]
    fn active_token_is_mutable() {
        with_contract(|env| {
            let (_, _, _) = seed(env);
            assert_eq!(royalty_state(env, TOKEN), Ok(RoyaltyLifecycleState::Active));
            assert!(validate_state_for_update(env, TOKEN).is_ok());
        });
    }

    #[test]
    fn missing_token_is_rejected() {
        with_contract(|env| {
            let (admin, _, _) = seed(env);
            let _ = admin;
            let missing = 999;
            assert_eq!(royalty_state(env, missing), Err(Error::TokenNotFound));
            assert_eq!(
                validate_state_for_update(env, missing),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn frozen_token_rejects_updates() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            assert!(freeze_royalty(env, &creator, TOKEN).is_ok());
            assert_eq!(royalty_state(env, TOKEN), Ok(RoyaltyLifecycleState::Frozen));
            assert_eq!(
                validate_state_for_update(env, TOKEN),
                Err(Error::RoyaltyFrozen)
            );
        });
    }

    #[test]
    fn frozen_state_has_no_valid_outgoing_transition() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            assert!(freeze_royalty(env, &creator, TOKEN).is_ok());
            // A frozen configuration offers no path back to Active: every
            // transition attempt remains invalid.
            assert_eq!(royalty_state(env, TOKEN), Ok(RoyaltyLifecycleState::Frozen));
            assert_eq!(
                validate_state_for_update(env, TOKEN),
                Err(Error::RoyaltyFrozen)
            );
            // Repeatedly — the lifecycle is one-way and permanent.
            assert_eq!(royalty_state(env, TOKEN), Ok(RoyaltyLifecycleState::Frozen));
        });
    }
}
