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
    let data: TokenData =
        read_persistent(env, &DataKey::Token(token_id)).ok_or(Error::TokenNotFound)?;
    validate_token_data(&data)
}

/// Deserialize and validate a metadata URI for `token_id`.
pub fn deserialize_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    let uri: String =
        read_persistent(env, &DataKey::Metadata(token_id)).ok_or(Error::TokenNotFound)?;
    if uri.len() == 0 {
        return Err(Error::CorruptedStorage);
    }
    Ok(uri)
}

/// Deserialize and validate royalty config for `token_id`.
pub fn deserialize_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    let royalty: Royalty =
        read_persistent(env, &DataKey::Royalty(token_id)).ok_or(Error::TokenNotFound)?;
    validate_royalty(&royalty)
}

fn validate_token_data(data: &TokenData) -> Result<TokenData, Error> {
    Ok(data.clone())
}

fn validate_royalty(royalty: &Royalty) -> Result<Royalty, Error> {
    let mut total_bps: u32 = 0;
    for r in royalty.recipients.iter() {
        if r.basis_points > MAX_ROYALTY_BPS {
            return Err(Error::CorruptedStorage);
        }
        total_bps = total_bps.saturating_add(r.basis_points);
    }
    if total_bps > MAX_ROYALTY_BPS {
        return Err(Error::CorruptedStorage);
    }
    Ok(royalty.clone())
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
    fn deserialize_metadata_empty_fails_corrupted() {
        with_contract(|env| {
            let empty = String::from_str(env, "");
            env.storage()
                .persistent()
                .set(&DataKey::Metadata(1), &empty);
            assert_eq!(deserialize_metadata(env, 1), Err(Error::CorruptedStorage));
        });
    }

    #[test]
    fn deserialize_metadata_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(deserialize_metadata(env, 55), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn deserialize_royalty_corrupted_when_bps_too_high() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let royalty = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient,
                        basis_points: MAX_ROYALTY_BPS + 1
                    }
                ],
                asset_address: None,
            };
            env.storage()
                .persistent()
                .set(&DataKey::Royalty(3), &royalty);
            assert_eq!(deserialize_royalty(env, 3), Err(Error::CorruptedStorage));
        });
    }

    #[test]
    fn deserialize_token_missing_returns_not_found() {
        with_contract(|env| {
            assert!(matches!(
                deserialize_token(env, 99),
                Err(Error::TokenNotFound)
            ));
        });
    }
}
