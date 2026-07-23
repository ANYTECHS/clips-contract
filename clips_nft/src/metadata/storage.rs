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
/// Also maintains a metadata index at `DataKey::MetadataIndex(uri)` to prevent duplicates
///
/// # Example
/// ```rust,ignore
/// save_metadata(&env, 1, &String::from_str(&env, "ipfs://QmHash"));
/// ```
pub fn save_metadata(env: &Env, token_id: TokenId, uri: &String) {
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    // Maintain metadata index to prevent duplicate metadata URIs
    env.storage()
        .persistent()
        .set(&DataKey::MetadataIndex(uri.clone()), &token_id);
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
/// Also updates the metadata index to maintain uniqueness constraint.
pub fn update_metadata(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    if !env
        .storage()
        .persistent()
        .has(&DataKey::Metadata(token_id))
    {
        return Err(Error::TokenNotFound);
    }
    // Get old URI to remove from index
    let old_uri = env
        .storage()
        .persistent()
        .get::<DataKey, String>(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)?;
    // Remove old index entry
    env.storage()
        .persistent()
        .remove(&DataKey::MetadataIndex(old_uri));
    // Set new metadata and update index
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    env.storage()
        .persistent()
        .set(&DataKey::MetadataIndex(uri.clone()), &token_id);
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
/// Also removes the metadata index entry to maintain consistency.
pub fn remove_metadata(env: &Env, token_id: TokenId) {
    // Get URI before removing to clean up index
    if let Some(uri) = env
        .storage()
        .persistent()
        .get::<DataKey, String>(&DataKey::Metadata(token_id))
    {
        env.storage()
            .persistent()
            .remove(&DataKey::MetadataIndex(uri));
    }
    env.storage()
        .persistent()
        .remove(&DataKey::Metadata(token_id));
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would require proper Soroban test environment
    // Placeholder for when test infrastructure is set up
}
