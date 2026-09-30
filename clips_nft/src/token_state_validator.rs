//! NFT token-state validation.
//!
//! Validates the current lifecycle state of an NFT before executing
//! state-dependent logic. This keeps callers from acting on tokens that are
//! not in an allowed state, such as frozen or active tokens.

use soroban_sdk::Env;

use crate::frozen_token;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// The current lifecycle state of an NFT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenState {
    /// Token is active and can be used in normal flows.
    Active,
    /// Token is frozen and cannot be used in mutable flows.
    Frozen,
}

/// Return the current lifecycle state of `token_id`.
pub fn get_token_state(env: &Env, token_id: TokenId) -> Result<TokenState, Error> {
    token_owner_storage::get_owner(env, token_id)?;
    Ok(if frozen_token::is_frozen(env, token_id) {
        TokenState::Frozen
    } else {
        TokenState::Active
    })
}

/// Validate that `token_id` is in one of the allowed states.
pub fn validate_token_state(
    env: &Env,
    token_id: TokenId,
    allowed_states: &[TokenState],
) -> Result<(), Error> {
    let state = get_token_state(env, token_id)?;
    if allowed_states.iter().any(|allowed| *allowed == state) {
        Ok(())
    } else {
        Err(Error::Unauthorized)
    }
}

/// Alias for [`validate_token_state`].
pub fn require_token_state(
    env: &Env,
    token_id: TokenId,
    allowed_states: &[TokenState],
) -> Result<(), Error> {
    validate_token_state(env, token_id, allowed_states)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frozen_token::freeze_token;
    use crate::token_owner_storage::save_owner;
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
    fn resolves_active_token_state() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 1, &owner);

            assert_eq!(get_token_state(env, 1), Ok(TokenState::Active));
        });
    }

    #[test]
    fn resolves_frozen_token_state() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 2, &owner);
            freeze_token(env, 2);

            assert_eq!(get_token_state(env, 2), Ok(TokenState::Frozen));
        });
    }

    #[test]
    fn accepts_allowed_state() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 3, &owner);

            assert!(validate_token_state(env, 3, &[TokenState::Active]).is_ok());
            assert_eq!(
                validate_token_state(env, 3, &[TokenState::Frozen]),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn rejects_invalid_state() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 4, &owner);
            freeze_token(env, 4);

            assert_eq!(
                validate_token_state(env, 4, &[TokenState::Active]),
                Err(Error::Unauthorized)
            );
            assert!(validate_token_state(env, 4, &[TokenState::Frozen]).is_ok());
        });
    }

    #[test]
    fn missing_token_is_rejected() {
        with_contract(|env| {
            assert_eq!(get_token_state(env, 999), Err(Error::TokenNotFound));
            assert_eq!(
                validate_token_state(env, 999, &[TokenState::Active]),
                Err(Error::TokenNotFound)
            );
        });
    }
}
