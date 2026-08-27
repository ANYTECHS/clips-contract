//! Token storage repository — encapsulates all persistent token storage operations.

use crate::types::{DataKey, Error, Royalty, TokenData, TokenId};
use soroban_sdk::{Env, String};

/// Load token data. Returns `Err(TokenNotFound)` if absent.
pub fn get_token(env: &Env, token_id: TokenId) -> Result<TokenData, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Persist token data.
pub fn set_token(env: &Env, token_id: TokenId, data: &TokenData) {
    env.storage()
        .persistent()
        .set(&DataKey::Token(token_id), data);
}

/// Remove all persistent entries for a token.
/// Also removes the metadata index entry to maintain consistency.
pub fn remove_token(env: &Env, token_id: TokenId) {
    // Get metadata URI before removing to clean up index
    if let Some(uri) = env
        .storage()
        .persistent()
        .get::<DataKey, String>(&DataKey::Metadata(token_id))
    {
        env.storage()
            .persistent()
            .remove(&DataKey::MetadataIndex(uri));
    }
    env.storage().persistent().remove(&DataKey::Token(token_id));
    env.storage()
        .persistent()
        .remove(&DataKey::Metadata(token_id));
    env.storage()
        .persistent()
        .remove(&DataKey::Royalty(token_id));
}

/// Returns true if the token exists.
pub fn token_exists(env: &Env, token_id: TokenId) -> bool {
    env.storage().persistent().has(&DataKey::Token(token_id))
}

/// Load metadata URI. Returns `Err(TokenNotFound)` if absent.
pub fn get_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Persist metadata URI. Returns Err if metadata size exceeds limit.
/// Also maintains metadata index to prevent duplicate metadata URIs.
/// Persist metadata URI.
pub fn set_metadata(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    // Maintain metadata index to prevent duplicate metadata URIs
    env.storage()
        .persistent()
        .set(&DataKey::MetadataIndex(uri.clone()), &token_id);
    Ok(())
}

/// Load royalty config. Returns `Err(TokenNotFound)` if absent.
pub fn get_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Royalty(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Ensure a token exists, returning `Err(TokenNotFound)` otherwise.
///
/// Used to guard royalty assignment (issue #791) and other operations that
/// must not write state for nonexistent NFTs.
pub fn require_token_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if token_exists(env, token_id) {
        Ok(())
    } else {
        Err(Error::TokenNotFound)
    }
}

/// Persist royalty config.
pub fn set_royalty(env: &Env, token_id: TokenId, royalty: &Royalty) {
    env.storage()
        .persistent()
        .set(&DataKey::Royalty(token_id), royalty);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyRecipient;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn set_and_get_token_roundtrip() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let td = TokenData {
                owner: owner.clone(),
                clip_id: 7,
            };
            set_token(env, 7, &td);
            let got = get_token(env, 7).unwrap();
            assert_eq!(got.owner, owner);
            assert_eq!(got.clip_id, 7);
            assert!(token_exists(env, 7));
        });
    }

    #[test]
    fn get_token_missing_fails() {
        with_contract(|env| {
            assert!(matches!(get_token(env, 1234), Err(Error::TokenNotFound)));
            assert!(!token_exists(env, 1234));
        });
    }

    #[test]
    fn set_and_get_metadata_and_indexing() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://Qm...");
            set_metadata(env, 1, &uri).unwrap();
            assert_eq!(get_metadata(env, 1).unwrap(), uri.clone());
            // Metadata index should map uri -> token id; when removing token the index is cleaned
            let td = TokenData {
                owner: Address::generate(env),
                clip_id: 1,
            };
            set_token(env, 1, &td);
            remove_token(env, 1);
            assert!(matches!(get_token(env, 1), Err(Error::TokenNotFound)));
            assert_eq!(get_metadata(env, 1), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn set_and_get_royalty() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let royalty = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: recipient.clone(),
                        basis_points: 250
                    }
                ],
                asset_address: None,
            };
            set_royalty(env, 2, &royalty);
            let got = get_royalty(env, 2).unwrap();
            assert_eq!(got.recipients.get(0).unwrap().basis_points, 250);
            assert_eq!(got.recipients.get(0).unwrap().recipient, recipient);
        });
    }
}
