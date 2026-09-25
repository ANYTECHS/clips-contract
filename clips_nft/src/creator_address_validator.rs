//! Creator address validator (issue #1092).
//!
//! Validates creator addresses associated with NFTs.
//!
//! [`crate::creator_storage::assign_creator`] currently writes whatever it is
//! given and emits the event, so every rule about *which* creator may be
//! recorded lives here:
//!
//! * **Address** — a Soroban `Address` is structurally valid by construction, so
//!   there is no format to parse. Rejecting a string that cannot be parsed is
//!   not possible inside the contract either: `Address::from_string` panics
//!   rather than returning an error. The checks that carry information are the
//!   two this crate already applies elsewhere — the NFT contract cannot be its
//!   own creator ([`crate::token_owner_storage::validate_owner`], used by the
//!   mint pipeline) and a blacklisted wallet cannot be a creator
//!   ([`crate::blacklist::is_blacklisted`], the rule `mint_validator` applies at
//!   line 84).
//! * **Association** — a token either has a creator record or it does not;
//!   "does not" is [`Error::EmptyCreator`] rather than a missing key.
//! * **Assignments** — a creator is written once. Re-assigning the *same*
//!   creator is idempotent (a retried mint must not fail), while replacing it
//!   with a different wallet is refused until there is a reassignment flow with
//!   its own rules. That policy is a choice, not a deduction — say the word and
//!   the error (or the allowance) changes shape.
//!
//! ```ignore
//! creator_address_validator::validate_creator_assignment(&env, token_id, &creator)?;
//! creator_storage::assign_creator(&env, token_id, &creator, clip_id)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::creator_storage;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// Reject a creator address that may never own the association.
///
/// # Errors
/// * [`Error::InvalidAddress`] — `creator` is the NFT contract's own address.
/// * [`Error::Unauthorized`] — `creator` is blacklisted.
pub fn validate_creator_address(env: &Env, creator: &Address) -> Result<(), Error> {
    // Same rule the mint pipeline applies to owners.
    token_owner_storage::validate_owner(env, creator)?;

    if blacklist::is_blacklisted(env, creator) {
        return Err(Error::Unauthorized);
    }

    Ok(())
}

/// Return the creator recorded for a token.
///
/// # Errors
/// * [`Error::EmptyCreator`] — the token has no creator record
///   ([`creator_storage::get_creator`] reports this as `TokenNotFound`, which
///   reads as "the token does not exist" for a caller that is holding a token).
pub fn require_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    creator_storage::get_creator(env, token_id).map_err(|_| Error::EmptyCreator)
}

/// Validate that `creator` may be recorded as the creator of `token_id`.
///
/// # Errors
/// * [`Error::InvalidAddress`] / [`Error::Unauthorized`] — see
///   [`validate_creator_address`].
/// * [`Error::Unauthorized`] — the token already has a different creator.
pub fn validate_creator_assignment(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
) -> Result<(), Error> {
    validate_creator_address(env, creator)?;

    if let Ok(existing) = require_creator(env, token_id) {
        if existing != *creator {
            return Err(Error::Unauthorized);
        }
    }

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
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn contract_own_address_is_rejected() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(
                validate_creator_address(env, &contract),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn blacklisted_creator_is_rejected() {
        with_contract(|env| {
            let creator = Address::generate(env);
            blacklist::add_wallet(env, &creator);

            assert_eq!(
                validate_creator_address(env, &creator),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn ordinary_address_is_accepted() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert_eq!(validate_creator_address(env, &creator), Ok(()));
        });
    }

    #[test]
    fn token_without_a_creator_record_reports_empty_creator() {
        with_contract(|env| {
            assert_eq!(require_creator(env, 1), Err(Error::EmptyCreator));
            // The storage helper calls the same state TokenNotFound, which is
            // the difference this validator exists to make explicit.
            assert_eq!(
                creator_storage::get_creator(env, 1),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn stored_creator_is_returned() {
        with_contract(|env| {
            let creator = Address::generate(env);
            creator_storage::set_creator(env, 2, &creator);

            assert_eq!(require_creator(env, 2), Ok(creator.clone()));
            assert_eq!(validate_creator_assignment(env, 2, &creator), Ok(()));
        });
    }

    #[test]
    fn first_assignment_is_accepted() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert_eq!(validate_creator_assignment(env, 3, &creator), Ok(()));
        });
    }

    #[test]
    fn replacive_assignment_is_rejected() {
        with_contract(|env| {
            let first = Address::generate(env);
            let second = Address::generate(env);
            creator_storage::set_creator(env, 4, &first);

            assert_eq!(
                validate_creator_assignment(env, 4, &second),
                Err(Error::Unauthorized)
            );
            // The recorded creator is unchanged by the rejected attempt.
            assert_eq!(require_creator(env, 4), Ok(first));
        });
    }

    #[test]
    fn reassigning_the_same_creator_is_idempotent() {
        with_contract(|env| {
            let creator = Address::generate(env);
            creator_storage::set_creator(env, 5, &creator);

            // A retried mint must not fail on the second pass.
            assert_eq!(validate_creator_assignment(env, 5, &creator), Ok(()));
        });
    }

    #[test]
    fn blacklisted_creator_cannot_be_assigned_to_an_existing_token() {
        with_contract(|env| {
            let creator = Address::generate(env);
            creator_storage::set_creator(env, 6, &creator);
            blacklist::add_wallet(env, &creator);

            assert_eq!(
                validate_creator_assignment(env, 6, &creator),
                Err(Error::Unauthorized)
            );
        });
    }
}
