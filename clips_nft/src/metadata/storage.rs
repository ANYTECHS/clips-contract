//! Metadata storage operations.
//!
//! This module handles the persistence and retrieval of NFT metadata
//! in Soroban's storage layer.

use soroban_sdk::{Env, String};

use crate::types::{DataKey, TokenId};
use crate::errors::Error;

/// Persist the metadata URI for a token.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `token_id` - The token ID to store metadata for
/// * `uri` - The metadata URI to persist
///
/// # Storage
/// Uses persistent storage with key `DataKey::Metadata(token_id)`
///
/// # Example
/// ```rust,ignore
/// save_metadata(&env, 1, &String::from_str(&env, "ipfs://QmHash"));
/// ```
pub fn save_metadata(env: &Env, token_id: TokenId, uri: &String) {
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
}

/// Load the metadata URI for a token.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `token_id` - The token ID to retrieve metadata for
///
/// # Returns
/// - `Ok(String)` - The metadata URI if found
/// - `Err(Error::TokenNotFound)` - If no metadata exists for the token
///
/// # Example
/// ```rust,ignore
/// let uri = get_metadata(&env, 1)?;
/// ```
pub fn get_metadata(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Update the metadata URI for an existing token.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `token_id` - The token ID to update metadata for
/// * `uri` - The new metadata URI
///
/// # Returns
/// - `Ok(())` - If the update was successful
/// - `Err(Error::TokenNotFound)` - If no metadata exists for the token
///
/// # Example
/// ```rust,ignore
/// update_metadata(&env, 1, &String::from_str(&env, "ipfs://QmNewHash"))?;
/// ```
///
/// # Note
/// This function checks if metadata exists before updating to prevent
/// accidentally creating metadata for non-existent tokens.
pub fn update_metadata(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    if !env
        .storage()
        .persistent()
        .has(&DataKey::Metadata(token_id))
    {
        return Err(Error::TokenNotFound);
    }
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    Ok(())
}

/// Check if metadata exists for a token.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `token_id` - The token ID to check
///
/// # Returns
/// `true` if metadata exists, `false` otherwise
///
/// # Example
/// ```rust,ignore
/// if metadata_exists(&env, 1) {
///     // Token has metadata
/// }
/// ```
pub fn metadata_exists(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Metadata(token_id))
}

/// Remove metadata for a token (used during burn operations).
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `token_id` - The token ID to remove metadata for
///
/// # Example
/// ```rust,ignore
/// remove_metadata(&env, 1);
/// ```
///
/// # Note
/// This is typically called as part of the token burn process.
pub fn remove_metadata(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Metadata(token_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    // ========== save_metadata and get_metadata tests ==========

    #[test]
    fn test_save_and_get_metadata() {
        let env = Env::default();
        let token_id = 1u32;
        let uri = String::from_str(&env, "ipfs://QmTestHash");

        save_metadata(&env, token_id, &uri);
        let retrieved = get_metadata(&env, token_id);

        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), uri);
    }

    #[test]
    fn test_get_metadata_not_found() {
        let env = Env::default();
        let token_id = 999u32;

        let result = get_metadata(&env, token_id);
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    #[test]
    fn test_save_metadata_overwrites_existing() {
        let env = Env::default();
        let token_id = 2u32;
        let uri1 = String::from_str(&env, "ipfs://QmFirstHash");
        let uri2 = String::from_str(&env, "ipfs://QmSecondHash");

        save_metadata(&env, token_id, &uri1);
        save_metadata(&env, token_id, &uri2);

        let retrieved = get_metadata(&env, token_id).unwrap();
        assert_eq!(retrieved, uri2);
    }

    // ========== metadata_exists tests ==========

    #[test]
    fn test_metadata_exists_true() {
        let env = Env::default();
        let token_id = 3u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        save_metadata(&env, token_id, &uri);
        assert!(metadata_exists(&env, token_id));
    }

    #[test]
    fn test_metadata_exists_false() {
        let env = Env::default();
        let token_id = 999u32;

        assert!(!metadata_exists(&env, token_id));
    }

    #[test]
    fn test_metadata_exists_after_removal() {
        let env = Env::default();
        let token_id = 4u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        save_metadata(&env, token_id, &uri);
        assert!(metadata_exists(&env, token_id));

        remove_metadata(&env, token_id);
        assert!(!metadata_exists(&env, token_id));
    }

    // ========== update_metadata tests ==========

    #[test]
    fn test_update_metadata_success() {
        let env = Env::default();
        let token_id = 5u32;
        let uri1 = String::from_str(&env, "ipfs://QmOldHash");
        let uri2 = String::from_str(&env, "ipfs://QmNewHash");

        save_metadata(&env, token_id, &uri1);
        let result = update_metadata(&env, token_id, &uri2);

        assert!(result.is_ok());

        let retrieved = get_metadata(&env, token_id).unwrap();
        assert_eq!(retrieved, uri2);
    }

    #[test]
    fn test_update_metadata_not_found_fails() {
        let env = Env::default();
        let token_id = 999u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        let result = update_metadata(&env, token_id, &uri);
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    // ========== remove_metadata tests ==========

    #[test]
    fn test_remove_metadata_success() {
        let env = Env::default();
        let token_id = 6u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        save_metadata(&env, token_id, &uri);
        assert!(metadata_exists(&env, token_id));

        remove_metadata(&env, token_id);
        assert!(!metadata_exists(&env, token_id));
    }

    #[test]
    fn test_remove_metadata_not_exists() {
        let env = Env::default();
        let token_id = 999u32;

        // Should not panic when removing non-existent metadata
        remove_metadata(&env, token_id);
        assert!(!metadata_exists(&env, token_id));
    }

    #[test]
    fn test_remove_metadata_then_get_fails() {
        let env = Env::default();
        let token_id = 7u32;
        let uri = String::from_str(&env, "ipfs://QmHash");

        save_metadata(&env, token_id, &uri);
        remove_metadata(&env, token_id);

        let result = get_metadata(&env, token_id);
        assert_eq!(result, Err(Error::TokenNotFound));
    }

    // ========== Multiple tokens tests ==========

    #[test]
    fn test_multiple_tokens_independent_storage() {
        let env = Env::default();
        let uri1 = String::from_str(&env, "ipfs://QmToken1");
        let uri2 = String::from_str(&env, "ipfs://QmToken2");
        let uri3 = String::from_str(&env, "ipfs://QmToken3");

        save_metadata(&env, 1, &uri1);
        save_metadata(&env, 2, &uri2);
        save_metadata(&env, 3, &uri3);

        assert_eq!(get_metadata(&env, 1).unwrap(), uri1);
        assert_eq!(get_metadata(&env, 2).unwrap(), uri2);
        assert_eq!(get_metadata(&env, 3).unwrap(), uri3);
    }

    #[test]
    fn test_update_one_token_does_not_affect_others() {
        let env = Env::default();
        let uri1 = String::from_str(&env, "ipfs://QmToken1");
        let uri2 = String::from_str(&env, "ipfs://QmToken2");
        let uri2_new = String::from_str(&env, "ipfs://QmToken2Updated");

        save_metadata(&env, 1, &uri1);
        save_metadata(&env, 2, &uri2);

        update_metadata(&env, 2, &uri2_new).unwrap();

        assert_eq!(get_metadata(&env, 1).unwrap(), uri1);
        assert_eq!(get_metadata(&env, 2).unwrap(), uri2_new);
    }

    #[test]
    fn test_remove_one_token_does_not_affect_others() {
        let env = Env::default();
        let uri1 = String::from_str(&env, "ipfs://QmToken1");
        let uri2 = String::from_str(&env, "ipfs://QmToken2");

        save_metadata(&env, 1, &uri1);
        save_metadata(&env, 2, &uri2);

        remove_metadata(&env, 1);

        assert!(!metadata_exists(&env, 1));
        assert!(metadata_exists(&env, 2));
        assert_eq!(get_metadata(&env, 2).unwrap(), uri2);
    }
}
