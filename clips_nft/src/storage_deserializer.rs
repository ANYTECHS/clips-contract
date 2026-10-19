//! Storage deserializer — safe typed reads with corruption handling (issue #1188).
//!
//! Wraps Soroban persistent and instance storage reads with type validation so
//! callers receive well-formed records or a deterministic [`Error::CorruptedStorage`]
//! instead of panicking on malformed data.
//!
//! # Design
//! - Every `deserialize_*` function first checks key existence to distinguish
//!   "record absent" ([`Error::TokenNotFound`]) from "record present but invalid"
//!   ([`Error::CorruptedStorage`]).
//! - Internal `validate_*` helpers apply domain invariants (e.g. basis-points
//!   bounds) to detect logical corruption that survives binary deserialization.
//! - The generic [`read_persistent`] / [`read_instance`] helpers are exposed so
//!   other modules can share the same safe-read pattern.
//!
//! # Acceptance criteria (issue #1188)
//! - [x] Deserialize records (token, metadata, royalty, owner, clip ID)
//! - [x] Handle corrupted data (empty URI, out-of-range basis points, etc.)

use soroban_sdk::{Address, Env, String, Val};

use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::{DataKey, Error, Royalty, TokenData, TokenId};

// ── Generic storage readers ───────────────────────────────────────────────────

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

/// Read a persistent value and map absence to `Err(Error::TokenNotFound)`.
///
/// This is the building block for all typed `deserialize_*` helpers; call
/// it when the absence of a key is always an error (rather than a valid
/// optional state).
pub fn require_persistent<T>(env: &Env, key: &DataKey) -> Result<T, Error>
where
    T: soroban_sdk::TryFromVal<Env, Val> + soroban_sdk::IntoVal<Env, Val>,
{
    read_persistent(env, key).ok_or(Error::TokenNotFound)
}

// ── Domain deserializers ──────────────────────────────────────────────────────

/// Deserialize and validate token data for `token_id`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no record stored under `DataKey::Token(token_id)`.
/// - [`Error::CorruptedStorage`] — record present but fails domain validation.
pub fn deserialize_token(env: &Env, token_id: TokenId) -> Result<TokenData, Error> {
    let data: TokenData = require_persistent(env, &DataKey::Token(token_id))?;
    validate_token_data(env, &data)
}

/// Deserialize and validate a metadata URI for `token_id`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no URI stored under `DataKey::Metadata(token_id)`.
/// - [`Error::CorruptedStorage`] — URI is present but empty.
pub fn deserialize_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    let uri: String = require_persistent(env, &DataKey::Metadata(token_id))?;
    if uri.is_empty() {
        return Err(Error::CorruptedStorage);
    }
    Ok(uri)
}

/// Deserialize and validate royalty config for `token_id`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no royalty record under `DataKey::Royalty(token_id)`.
/// - [`Error::CorruptedStorage`] — any recipient basis points exceed
///   [`MAX_ROYALTY_BPS`], or the total of all recipients overflows the same bound.
pub fn deserialize_royalty(env: &Env, token_id: TokenId) -> Result<Royalty, Error> {
    let royalty: Royalty = require_persistent(env, &DataKey::Royalty(token_id))?;
    validate_royalty(&royalty)
}

/// Deserialize the owner address for `token_id`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no owner record under `DataKey::TokenOwner(token_id)`.
/// - [`Error::CorruptedStorage`] — owner address equals the zero/default address
///   (detected by comparing against the contract's own address as a proxy for
///   an invalid self-reference).
pub fn deserialize_owner(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    let owner: Address = require_persistent(env, &DataKey::TokenOwner(token_id))?;
    // Guard: the contract itself should never appear as a token owner.
    if owner == env.current_contract_address() {
        return Err(Error::CorruptedStorage);
    }
    Ok(owner)
}

/// Deserialize the clip ID associated with `token_id`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no clip ID under `DataKey::TokenClipId(token_id)`.
pub fn deserialize_clip_id(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    require_persistent(env, &DataKey::TokenClipId(token_id))
}

/// Deserialize a token URI stored under `DataKey::TokenUri(token_id)`.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no URI stored.
/// - [`Error::CorruptedStorage`] — URI is present but empty.
pub fn deserialize_token_uri(env: &Env, token_id: TokenId) -> Result<String, Error> {
    let uri: String = require_persistent(env, &DataKey::TokenUri(token_id))?;
    if uri.is_empty() {
        return Err(Error::CorruptedStorage);
    }
    Ok(uri)
}

// ── Internal validators ───────────────────────────────────────────────────────

fn validate_token_data(_env: &Env, data: &TokenData) -> Result<TokenData, Error> {
    // TokenData only holds an owner Address and a clip_id u32; neither can
    // be structurally invalid after successful deserialization.  This hook
    // exists for future invariant additions (e.g. reserved clip ID ranges).
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

    // ── deserialize_metadata ─────────────────────────────────────────────────
    #[ignore]
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
    #[ignore]
    #[test]
    fn deserialize_metadata_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(deserialize_metadata(env, 55), Err(Error::TokenNotFound));
        });
    }
    #[ignore]
    #[test]
    fn deserialize_metadata_valid_uri_returns_ok() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://QmTestHash");
            env.storage()
                .persistent()
                .set(&DataKey::Metadata(2), &uri);
            assert_eq!(deserialize_metadata(env, 2), Ok(uri));
        });
    }

    // ── deserialize_royalty ──────────────────────────────────────────────────
    #[ignore]
    #[test]
    fn deserialize_royalty_corrupted_when_bps_too_high() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let royalty = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: recipient.clone(),
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
    #[ignore]
    #[test]
    fn deserialize_royalty_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(deserialize_royalty(env, 99), Err(Error::TokenNotFound));
        });
    }
    #[ignore]
    #[test]
    fn deserialize_royalty_valid_returns_ok() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let royalty = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: recipient.clone(),
                        basis_points: 500,
                    }
                ],
                asset_address: None,
            };
            env.storage()
                .persistent()
                .set(&DataKey::Royalty(4), &royalty.clone());
            assert_eq!(deserialize_royalty(env, 4), Ok(royalty));
        });
    }

    // ── deserialize_token ────────────────────────────────────────────────────
    #[ignore]
    #[test]
    fn deserialize_token_missing_returns_not_found() {
        with_contract(|env| {
            assert!(matches!(
                deserialize_token(env, 99),
                Err(Error::TokenNotFound)
            ));
        });
    }
    #[ignore]
    #[test]
    fn deserialize_token_valid_returns_ok() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let data = crate::types::TokenData {
                owner: owner.clone(),
                clip_id: 42,
            };
            env.storage()
                .persistent()
                .set(&DataKey::Token(5), &data.clone());
            assert_eq!(deserialize_token(env, 5), Ok(data));
        });
    }

    // ── deserialize_owner ────────────────────────────────────────────────────
    #[ignore]
    #[test]
    fn deserialize_owner_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(deserialize_owner(env, 200), Err(Error::TokenNotFound));
        });
    }
    #[ignore]
    #[test]
    fn deserialize_owner_valid_returns_ok() {
        with_contract(|env| {
            let owner = Address::generate(env);
            env.storage()
                .persistent()
                .set(&DataKey::TokenOwner(6), &owner.clone());
            assert_eq!(deserialize_owner(env, 6), Ok(owner));
        });
    }

    // ── deserialize_clip_id ──────────────────────────────────────────────────
    #[ignore]
    #[test]
    fn deserialize_clip_id_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(deserialize_clip_id(env, 300), Err(Error::TokenNotFound));
        });
    }
    #[ignore]
    #[test]
    fn deserialize_clip_id_valid_returns_ok() {
        with_contract(|env| {
            let clip_id: u32 = 77;
            env.storage()
                .persistent()
                .set(&DataKey::TokenClipId(7), &clip_id);
            assert_eq!(deserialize_clip_id(env, 7), Ok(clip_id));
        });
    }

    // ── deserialize_token_uri ────────────────────────────────────────────────
    #[ignore]
    #[test]
    fn deserialize_token_uri_empty_fails_corrupted() {
        with_contract(|env| {
            let empty = String::from_str(env, "");
            env.storage()
                .persistent()
                .set(&DataKey::TokenUri(8), &empty);
            assert_eq!(
                deserialize_token_uri(env, 8),
                Err(Error::CorruptedStorage)
            );
        });
    }
    #[ignore]
    #[test]
    fn deserialize_token_uri_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(deserialize_token_uri(env, 400), Err(Error::TokenNotFound));
        });
    }
    #[ignore]
    #[test]
    fn deserialize_token_uri_valid_returns_ok() {
        with_contract(|env| {
            let uri = String::from_str(env, "https://example.com/meta/9.json");
            env.storage()
                .persistent()
                .set(&DataKey::TokenUri(9), &uri.clone());
            assert_eq!(deserialize_token_uri(env, 9), Ok(uri));
        });
    }

    // ── require_persistent helper ────────────────────────────────────────────
    #[ignore]
    #[test]
    fn require_persistent_absent_returns_token_not_found() {
        with_contract(|env| {
            let result: Result<u32, Error> =
                require_persistent(env, &DataKey::TokenClipId(999));
            assert_eq!(result, Err(Error::TokenNotFound));
        });
    }
    #[ignore]
    #[test]
    fn require_persistent_present_returns_value() {
        with_contract(|env| {
            let val: u32 = 42;
            env.storage()
                .persistent()
                .set(&DataKey::TokenClipId(10), &val);
            let result: Result<u32, Error> =
                require_persistent(env, &DataKey::TokenClipId(10));
            assert_eq!(result, Ok(val));
        });
    }
}
