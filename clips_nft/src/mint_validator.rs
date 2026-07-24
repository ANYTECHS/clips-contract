//! Mint validator — validates mint requests before NFT creation.
//!
//! Checks clip dedup, metadata URI presence, and blacklist status.

use soroban_sdk::{Address, Env, String};

use crate::mint_authorization::require_mint_auth;
use crate::types::{DataKey, Error, Royalty};
use crate::metadata::validation::validate_metadata_uri;
use crate::royalty_validator::validate_royalty;

/// Validate a mint request before any state is written.
///
/// # Checks
/// 1. `clip_id` has not already been minted.
/// 2. `metadata_uri` is non-empty.
/// 3. Owner / creator wallet is not blacklisted.
pub fn validate_mint(
    env: &Env,
    clip_id: u32,
    metadata_uri: &String,
    owner: &Address,
) -> Result<(), Error> {
    if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
        return Err(Error::ClipAlreadyMinted);
    }

    if metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(owner.clone()))
        .unwrap_or(false)
    {
        return Err(Error::Unauthorized);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
