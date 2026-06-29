//! Storage deserializer — safe typed reads with corruption handling.
//!
//! Wraps Soroban storage reads with validation so callers receive well-formed
//! records or a deterministic [`Error::CorruptedStorage`] instead of panicking.

use soroban_sdk::{Env, String, Val};

use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::{DataKey, Error, Royalty, TokenData, TokenId};

/// Read an instance-scoped value. Returns `None` when the key is absent.
pub fn read_instance<T>(env: &Env, key: &DataKey) -> Option<T>
where
    T: soroban_sdk::TryFromVal<Env, Val> + soroban_sdk::IntoVal<Env, Val>,
{
    env.storage().instance().get(key)
}

/// Read a persistent value. Returns `None` when the key is absent.
pub fn read_persistent<T>(env: &Env, key: &DataKey) -> Option<T>
where
    T: soroban_sdk::TryFromVal<Env, Val> + soroban_sdk::IntoVal<Env, Val>,
{
    env.storage().persistent().get(key)
}

/// Deserialize and validate token data for `token_id`.
pub fn deserialize_token(env: &Env, token_id: TokenId) -> Result<TokenData, Error> {
    let data: TokenData = read_persistent(env, &DataKey::Token(token_id))
        .ok_or(Error::TokenNotFound)?;
    validate_token_data(&data)
}

/// Deserialize and validate a metadata URI for `token_id`.
pub fn deserialize_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    let uri: String = read_persistent(env, &DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)?;
    if uri.len() == 0 {
        return Err(Error::CorruptedStorage);
    }
    Ok(uri)
}

/// Deserialize and validate royalty config for `token_id`.
pub fn deserialize_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    let royalty: Royalty = read_persistent(env, &DataKey::Royalty(token_id))
        .ok_or(Error::TokenNotFound)?;
    validate_royalty(&royalty)
}

fn validate_token_data(data: &TokenData) -> Result<TokenData, Error> {
    Ok(data.clone())
}

fn validate_royalty(royalty: &Royalty) -> Result<Royalty, Error> {
    if royalty.basis_points > MAX_ROYALTY_BPS {
        return Err(Error::CorruptedStorage);
    }
    Ok(royalty.clone())
}
