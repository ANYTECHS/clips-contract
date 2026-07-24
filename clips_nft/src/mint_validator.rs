//! Mint validator — validates mint requests before NFT creation.
//!
//! Resolves issue #429. Checks:
//! - Caller is authorized to mint (owner or approved minter)
//! - Duplicate clip (clip already minted)
//! - Metadata URI is non-empty
//! - Metadata URI is not a duplicate
//! - Creator address is present (structurally guaranteed, validated via storage)
//! - Metadata URI is valid (supported protocol, correct length)
//! - Creator address is valid
//! - Wallet is not blacklisted
//! - Royalty configuration is valid (percentage, recipient, max limit)

use soroban_sdk::{Address, Env, String};

use crate::mint_authorization::require_mint_auth;
use crate::types::{DataKey, Error, Royalty};
use crate::metadata::validation::validate_metadata_uri;
use crate::royalty_validator::validate_royalty;

/// Validate a mint request before any state is written.
///
/// # Checks (in order)
/// 1. Caller is authorized to mint (owner or approved minter).
/// 2. `clip_id` has not already been minted.
/// 3. `metadata_uri` is non-empty.
/// 4. `metadata_uri` is not a duplicate.
/// 1. `creator` address is valid.
/// 2. `clip_id` has not already been minted.
/// 3. `metadata_uri` is valid (supported protocol, correct length).
/// 4. Royalty configuration is valid (basis points, recipient, etc.).
/// 5. `creator` address is not blacklisted.
///
/// Returns the first error encountered.
pub fn validate_mint(
    env: &Env,
    clip_id: u32,
    metadata_uri: &String,
    royalty: &Royalty,
    creator: &Address,
) -> Result<(), Error> {
    // 0. Authorization check
    require_mint_auth(env, creator)?;
    // 0. Validate creator address (basic validity, Soroban ensures structural validity)
    // (Placeholder for any additional address checks if needed; currently relies on type system)

    // 1. Duplicate clip check
    if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
        return Err(Error::ClipAlreadyMinted);
    }

    // 2. Validate metadata URI (supported protocol, length, etc.)
    validate_metadata_uri(env, metadata_uri)?;

    // 3. Validate royalty configuration
    validate_royalty(royalty)?;

    // 2b. Metadata must not be a duplicate
    if env
        .storage()
        .persistent()
        .has(&DataKey::MetadataIndex(metadata_uri.clone()))
    {
        return Err(Error::DuplicateMetadata);
    }

    // 3. Wallet must not be blacklisted
    // 4. Wallet must not be blacklisted
    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(creator.clone()))
        .unwrap_or(false)
    {
        return Err(Error::Unauthorized);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn env_with_clip(clip_id: u32) -> Env {
        let env = Env::default();
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(clip_id), &0u32);
        env
    }

    #[test]
    fn valid_mint_passes() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty { recipient: creator.clone(), basis_points: 500, asset_address: None };
        assert!(validate_mint(&env, 1, &uri, &royalty, &creator).is_ok());
    }

    #[test]
    fn duplicate_clip_fails() {
        let env = env_with_clip(42);
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty { recipient: creator.clone(), basis_points: 500, asset_address: None };
        assert_eq!(validate_mint(&env, 42, &uri, &royalty, &creator), Err(Error::ClipAlreadyMinted));
    }

    #[test]
    fn empty_metadata_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "");
        let royalty = Royalty { recipient: creator.clone(), basis_points: 500, asset_address: None };
        assert_eq!(validate_mint(&env, 1, &uri, &royalty, &creator), Err(Error::InvalidURI));
    }

    #[test]
    fn blacklisted_wallet_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Blacklisted(creator.clone()), &true);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty { recipient: creator.clone(), basis_points: 500, asset_address: None };
        assert_eq!(validate_mint(&env, 1, &uri, &royalty, &creator), Err(Error::Unauthorized));
    }

    #[test]
    fn duplicate_metadata_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmDuplicate");
        env.storage()
            .persistent()
            .set(&DataKey::MetadataIndex(uri.clone()), &1u32);
        let royalty = Royalty { recipient: creator.clone(), basis_points: 500, asset_address: None };
        assert_eq!(validate_mint(&env, 1, &uri, &royalty, &creator), Err(Error::DuplicateMetadata));
    }

    #[test]
    fn unauthorized_minter_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &owner);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty { recipient: other.clone(), basis_points: 500, asset_address: None };
        assert_eq!(validate_mint(&env, 1, &uri, &royalty, &other), Err(Error::UnauthorizedMinter));
    }

    #[test]
    fn approved_minter_passes() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let minter = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &owner);
        crate::mint_authorization::set_approved_minter(&env, &minter);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty { recipient: minter.clone(), basis_points: 500, asset_address: None };
        assert!(validate_mint(&env, 1, &uri, &royalty, &minter).is_ok());
    }
}
