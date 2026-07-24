//! Persist metadata URIs for minted NFTs (issue #667).
//!
//! Stores the metadata URI for every minted NFT so off-chain clients can
//! retrieve metadata. Accepts IPFS (`ipfs://`) and HTTPS (`https://`) URIs
//! (Arweave `ar://` is also accepted for compatibility with the rest of the
//! contract). Validates the URI before writing and verifies the stored value
//! after persistence.
//!
//! # Storage
//! - `DataKey::TokenUri(token_id)` → `String`
//! - `DataKey::Metadata(token_id)` → `String` (canonical metadata URI)
//! - `DataKey::MetadataIndex(uri)` → `TokenId` (uniqueness index)

use soroban_sdk::{Env, String};

use crate::metadata_uri_validator::validate_metadata_uri;
use crate::types::{DataKey, Error, TokenId};

/// Persist a validated metadata URI for `token_id`.
///
/// # Errors
/// - [`Error::InvalidURI`] for empty or unsupported-protocol URIs.
/// - [`Error::CorruptedStorage`] if the value read back after write does not match.
pub fn persist_metadata_uri(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    validate_metadata_uri(uri)?;

    env.storage()
        .persistent()
        .set(&DataKey::TokenUri(token_id), uri);
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    env.storage()
        .persistent()
        .set(&DataKey::MetadataIndex(uri.clone()), &token_id);

    // Validate storage: confirm the persisted value matches the input.
    let stored: String = env
        .storage()
        .persistent()
        .get(&DataKey::TokenUri(token_id))
        .ok_or(Error::CorruptedStorage)?;
    if stored != *uri {
        return Err(Error::CorruptedStorage);
    }

    Ok(())
}

/// Read the persisted metadata URI for `token_id`.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if no URI has been stored.
pub fn get_persisted_metadata_uri(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenUri(token_id))
        .ok_or(Error::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{Env, String};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn uri(env: &Env, s: &str) -> String {
        String::from_str(env, s)
    }

    #[test]
    fn saves_ipfs_uri() {
        with_contract(|env| {
            let u = uri(env, "ipfs://QmClipHash123");
            persist_metadata_uri(env, 1, &u).unwrap();
            assert_eq!(get_persisted_metadata_uri(env, 1).unwrap(), u);

            let meta: String = env
                .storage()
                .persistent()
                .get(&DataKey::Metadata(1))
                .unwrap();
            assert_eq!(meta, u);
        });
    }

    #[test]
    fn saves_https_uri() {
        with_contract(|env| {
            let u = uri(env, "https://cdn.clipcash.example/meta/42.json");
            persist_metadata_uri(env, 2, &u).unwrap();
            assert_eq!(get_persisted_metadata_uri(env, 2).unwrap(), u);
        });
    }

    #[test]
    fn rejects_empty_uri() {
        with_contract(|env| {
            let u = uri(env, "");
            assert_eq!(persist_metadata_uri(env, 3, &u), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn rejects_unsupported_protocol() {
        with_contract(|env| {
            let u = uri(env, "ftp://files.example/meta.json");
            assert_eq!(persist_metadata_uri(env, 4, &u), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn rejects_plain_http() {
        with_contract(|env| {
            let u = uri(env, "http://insecure.example/meta.json");
            assert_eq!(persist_metadata_uri(env, 5, &u), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn storage_round_trip_matches_input() {
        with_contract(|env| {
            let u = uri(env, "ipfs://QmRoundTrip");
            persist_metadata_uri(env, 6, &u).unwrap();
            let stored = get_persisted_metadata_uri(env, 6).unwrap();
            assert_eq!(stored, u);

            let indexed: TokenId = env
                .storage()
                .persistent()
                .get(&DataKey::MetadataIndex(u.clone()))
                .unwrap();
            assert_eq!(indexed, 6);
        });
    }

    #[test]
    fn missing_uri_returns_not_found() {
        with_contract(|env| {
            assert_eq!(
                get_persisted_metadata_uri(env, 99),
                Err(Error::TokenNotFound)
            );
        });
    }
}
