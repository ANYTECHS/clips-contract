//! Link metadata records to newly minted NFTs (issue #666).
//!
//! Associates a generated NFT with its corresponding metadata record immediately
//! after minting. Metadata must be registered first so the link cannot point at
//! a missing / broken reference.
//!
//! # Storage
//! - `DataKey::MetadataRecord(uri)` → `bool` (registered metadata existence)
//! - `DataKey::Metadata(token_id)` → `String` (token → URI link)
//! - `DataKey::MetadataIndex(uri)` → `TokenId` (URI uniqueness / reverse lookup)
//!
//! # Errors
//! - [`Error::MetadataNotFound`] when linking to an unregistered metadata URI.
//! - [`Error::InvalidURI`] when the URI is empty.
//! - [`Error::DuplicateRecord`] when the token is already linked.

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Register a metadata record (by URI) so it can later be linked to an NFT.
///
/// Idempotent for the same URI. Rejects empty URIs.
pub fn register_metadata_record(env: &Env, uri: &String) -> Result<(), Error> {
    if uri.is_empty() {
        return Err(Error::InvalidURI);
    }
    env.storage()
        .persistent()
        .set(&DataKey::MetadataRecord(uri.clone()), &true);
    Ok(())
}

/// Return `true` if a metadata record has been registered for `uri`.
pub fn metadata_record_exists(env: &Env, uri: &String) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::MetadataRecord(uri.clone()))
        .unwrap_or(false)
}

/// Return `true` if `token_id` already has a linked metadata URI.
pub fn token_has_metadata_link(env: &Env, token_id: TokenId) -> bool {
    env.storage().persistent().has(&DataKey::Metadata(token_id))
}

/// Associate `token_id` with a previously registered metadata URI.
///
/// Validates that the metadata record exists before writing the link, which
/// prevents broken references from being persisted.
///
/// # Errors
/// - [`Error::InvalidURI`] if `uri` is empty.
/// - [`Error::MetadataNotFound`] if the metadata record was never registered.
/// - [`Error::DuplicateRecord`] if the token already has a metadata link.
pub fn link_metadata_to_nft(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    if uri.is_empty() {
        return Err(Error::InvalidURI);
    }
    if !metadata_record_exists(env, uri) {
        return Err(Error::MetadataNotFound);
    }
    if token_has_metadata_link(env, token_id) {
        return Err(Error::DuplicateRecord);
    }

    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    env.storage()
        .persistent()
        .set(&DataKey::MetadataIndex(uri.clone()), &token_id);

    Ok(())
}

/// Return the metadata URI linked to `token_id`.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if no link has been stored.
pub fn get_linked_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
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
    fn registers_and_checks_metadata_record() {
        with_contract(|env| {
            let u = uri(env, "ipfs://QmMeta1");
            assert!(!metadata_record_exists(env, &u));
            register_metadata_record(env, &u).unwrap();
            assert!(metadata_record_exists(env, &u));
        });
    }

    #[test]
    fn links_registered_metadata_to_nft() {
        with_contract(|env| {
            let u = uri(env, "ipfs://QmMeta2");
            register_metadata_record(env, &u).unwrap();
            link_metadata_to_nft(env, 10, &u).unwrap();

            assert_eq!(get_linked_metadata(env, 10).unwrap(), u);
            assert!(token_has_metadata_link(env, 10));

            let indexed: TokenId = env
                .storage()
                .persistent()
                .get(&DataKey::MetadataIndex(u.clone()))
                .unwrap();
            assert_eq!(indexed, 10);
        });
    }

    #[test]
    fn rejects_link_to_unregistered_metadata() {
        with_contract(|env| {
            let u = uri(env, "ipfs://QmMissing");
            assert_eq!(
                link_metadata_to_nft(env, 1, &u),
                Err(Error::MetadataNotFound)
            );
        });
    }

    #[test]
    fn prevents_broken_empty_uri_reference() {
        with_contract(|env| {
            let empty = uri(env, "");
            assert_eq!(
                register_metadata_record(env, &empty),
                Err(Error::InvalidURI)
            );
            assert_eq!(link_metadata_to_nft(env, 1, &empty), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn prevents_duplicate_token_link() {
        with_contract(|env| {
            let u = uri(env, "https://cdn.example/meta.json");
            register_metadata_record(env, &u).unwrap();
            link_metadata_to_nft(env, 5, &u).unwrap();
            assert_eq!(
                link_metadata_to_nft(env, 5, &u),
                Err(Error::DuplicateRecord)
            );
        });
    }

    #[test]
    fn unknown_token_has_no_link() {
        with_contract(|env| {
            assert_eq!(get_linked_metadata(env, 999), Err(Error::TokenNotFound));
            assert!(!token_has_metadata_link(env, 999));
        });
    }
}
