//! Token storage repository — encapsulates all persistent token storage operations.

use soroban_sdk::{Env, String};
use crate::types::{DataKey, Error, Royalty, TokenData, TokenId};

/// Load token data. Returns `Err(TokenNotFound)` if absent.
pub fn get_token(env: &Env, token_id: TokenId) -> Result<TokenData, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Persist token data.
pub fn set_token(env: &Env, token_id: TokenId, data: &TokenData) {
    env.storage().persistent().set(&DataKey::Token(token_id), data);
}

/// Remove all persistent entries for a token.
pub fn remove_token(env: &Env, token_id: TokenId) {
    env.storage().persistent().remove(&DataKey::Token(token_id));
    env.storage().persistent().remove(&DataKey::Metadata(token_id));
    env.storage().persistent().remove(&DataKey::Royalty(token_id));
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

/// Persist metadata URI.
pub fn set_metadata(env: &Env, token_id: TokenId, uri: &String) {
    env.storage().persistent().set(&DataKey::Metadata(token_id), uri);
}

/// Load royalty config. Returns `Err(TokenNotFound)` if absent.
pub fn get_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Royalty(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Persist royalty config.
pub fn set_royalty(env: &Env, token_id: TokenId, royalty: &Royalty) {
    env.storage().persistent().set(&DataKey::Royalty(token_id), royalty);
}
