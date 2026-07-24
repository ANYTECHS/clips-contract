//! NFT owner storage — assigns and verifies token ownership on mint.
//!
//! # Storage
//! Key: `DataKey::Token(token_id)` → `TokenData { owner, clip_id }` (persistent)

use soroban_sdk::{Address, Env};

use crate::token_storage;
use crate::types::{Error, TokenData, TokenId};

/// Reject owners that cannot hold NFTs (contract self-address).
pub fn validate_owner(env: &Env, owner: &Address) -> Result<(), Error> {
    let contract = env.current_contract_address();
    if *owner == contract {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Assign ownership of `token_id` to `owner` for `clip_id`.
pub fn assign_owner(
    env: &Env,
    token_id: TokenId,
    owner: &Address,
    clip_id: u32,
) -> Result<(), Error> {
    validate_owner(env, owner)?;
    let data = TokenData {
        owner: owner.clone(),
        clip_id,
    };
    token_storage::set_token(env, token_id, &data);
    Ok(())
}

/// Read the owner of `token_id`.
pub fn get_owner(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    Ok(token_storage::get_token(env, token_id)?.owner)
}

/// Verify `token_id` is owned by `expected`.
pub fn verify_owner(env: &Env, token_id: TokenId, expected: &Address) -> Result<(), Error> {
    let owner = get_owner(env, token_id)?;
    if owner != *expected {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Remove owner record during mint rollback.
pub fn remove_owner(env: &Env, token_id: TokenId) {
    token_storage::remove_token(env, token_id);
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
    fn assign_and_verify_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assign_owner(env, 0, &owner, 42).unwrap();
            verify_owner(env, 0, &owner).unwrap();
            assert_eq!(get_owner(env, 0).unwrap(), owner);
        });
    }

    #[test]
    fn rejects_contract_address_as_owner() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(crate::AtomicMintContract, ());
        env.as_contract(&contract_id, || {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                assign_owner(&env, 0, &contract_addr, 1),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn verify_owner_rejects_mismatch() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            assign_owner(env, 0, &owner, 1).unwrap();
            assert_eq!(verify_owner(env, 0, &other), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn get_owner_missing_token_fails() {
        with_contract(|env| {
            assert_eq!(get_owner(env, 99), Err(Error::TokenNotFound));
        });
    }
}
