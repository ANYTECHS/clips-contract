//! NFT ↔ collection association (issue #676).
//!
//! Registers newly minted NFTs under a ClipCash NFT collection. A collection
//! must be registered before tokens can be associated with it; each association
//! saves a token → collection reference and adds the token to the collection's
//! membership list.
//!
//! # Storage
//! - `DataKey::CollectionRegistered(collection_id)` → `bool` (existence marker)
//! - `DataKey::TokenCollection(token_id)` → `u32` (collection reference)
//! - `DataKey::CollectionMembers(collection_id)` → `Vec<TokenId>` (membership)
//!
//! All keys use persistent storage.

use soroban_sdk::{Env, Vec};

use crate::types::{DataKey, Error, TokenId};

/// Register a collection so tokens can be associated with it. Idempotent.
pub fn register_collection(env: &Env, collection_id: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::CollectionRegistered(collection_id), &true);
}

/// Return `true` if `collection_id` has been registered.
pub fn collection_exists(env: &Env, collection_id: u32) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::CollectionRegistered(collection_id))
        .unwrap_or(false)
}

/// Return `true` if `token_id` is already a member of `collection_id`.
pub fn collection_contains_token(env: &Env, collection_id: u32, token_id: TokenId) -> bool {
    get_collection_members(env, collection_id)
        .iter()
        .any(|t| t == token_id)
}

/// Associate `token_id` with `collection_id`.
///
/// Saves the token → collection reference and appends the token to the
/// collection's membership list.
///
/// # Errors
/// - [`Error::CollectionNotFound`] if the collection has not been registered.
/// - [`Error::DuplicateRecord`] if the token is already a member.
pub fn associate_nft(env: &Env, token_id: TokenId, collection_id: u32) -> Result<(), Error> {
    if !collection_exists(env, collection_id) {
        return Err(Error::CollectionNotFound);
    }
    if collection_contains_token(env, collection_id, token_id) {
        return Err(Error::DuplicateRecord);
    }

    env.storage()
        .persistent()
        .set(&DataKey::TokenCollection(token_id), &collection_id);

    let mut members = get_collection_members(env, collection_id);
    members.push_back(token_id);
    env.storage()
        .persistent()
        .set(&DataKey::CollectionMembers(collection_id), &members);

    Ok(())
}

/// Return the collection a token belongs to.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if the token is not associated with any collection.
pub fn get_nft_collection(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenCollection(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Retrieve every token ID that belongs to `collection_id`. Empty if none recorded.
pub fn get_collection_members(env: &Env, collection_id: u32) -> Vec<TokenId> {
    env.storage()
        .persistent()
        .get(&DataKey::CollectionMembers(collection_id))
        .unwrap_or_else(|| Vec::new(env))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::Env;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn register_and_check_collection() {
        with_contract(|env| {
            assert!(!collection_exists(env, 1));
            register_collection(env, 1);
            assert!(collection_exists(env, 1));
        });
    }

    #[test]
    fn associates_nft_with_registered_collection() {
        with_contract(|env| {
            register_collection(env, 1);
            associate_nft(env, 100, 1).unwrap();

            assert_eq!(get_nft_collection(env, 100).unwrap(), 1);
            let members = get_collection_members(env, 1);
            assert_eq!(members.len(), 1);
            assert_eq!(members.get(0).unwrap(), 100);
            assert!(collection_contains_token(env, 1, 100));
        });
    }

    #[test]
    fn rejects_association_with_unregistered_collection() {
        with_contract(|env| {
            assert_eq!(associate_nft(env, 100, 42), Err(Error::CollectionNotFound));
        });
    }

    #[test]
    fn prevents_duplicate_membership() {
        with_contract(|env| {
            register_collection(env, 1);
            associate_nft(env, 100, 1).unwrap();
            assert_eq!(associate_nft(env, 100, 1), Err(Error::DuplicateRecord));
            assert_eq!(get_collection_members(env, 1).len(), 1);
        });
    }

    #[test]
    fn tracks_multiple_members_in_order() {
        with_contract(|env| {
            register_collection(env, 1);
            associate_nft(env, 10, 1).unwrap();
            associate_nft(env, 20, 1).unwrap();
            associate_nft(env, 30, 1).unwrap();

            let members = get_collection_members(env, 1);
            assert_eq!(members.len(), 3);
            assert_eq!(members.get(0).unwrap(), 10);
            assert_eq!(members.get(2).unwrap(), 30);
        });
    }

    #[test]
    fn unknown_token_has_no_collection() {
        with_contract(|env| {
            assert_eq!(get_nft_collection(env, 999), Err(Error::TokenNotFound));
        });
    }
}
