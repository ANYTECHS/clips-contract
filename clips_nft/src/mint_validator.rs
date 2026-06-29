//! Mint validator — validates mint requests before NFT creation.
//!
//! Resolves issue #429. Checks:
//! - Duplicate clip (clip already minted)
//! - Metadata URI is non-empty
//! - Creator address is present (structurally guaranteed, validated via storage)
//! - Wallet is not blacklisted

use soroban_sdk::{Address, Env, String};

use crate::types::{DataKey, Error};

/// Validate a mint request before any state is written.
///
/// # Checks (in order)
/// 1. `clip_id` has not already been minted.
/// 2. `metadata_uri` is non-empty.
/// 3. `creator` address is not blacklisted.
///
/// Returns the first error encountered.
pub fn validate_mint(
    env: &Env,
    clip_id: u32,
    metadata_uri: &String,
    creator: &Address,
) -> Result<(), Error> {
    // 1. Duplicate clip check
    if env.storage().persistent().has(&DataKey::ClipIdMinted(clip_id)) {
        return Err(Error::ClipAlreadyMinted);
    }

    // 2. Metadata must be present
    if metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    // 3. Wallet must not be blacklisted
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
        assert!(validate_mint(&env, 1, &uri, &creator).is_ok());
    }

    #[test]
    fn duplicate_clip_fails() {
        let env = env_with_clip(42);
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmTest");
        assert_eq!(validate_mint(&env, 42, &uri, &creator), Err(Error::ClipAlreadyMinted));
    }

    #[test]
    fn empty_metadata_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "");
        assert_eq!(validate_mint(&env, 1, &uri, &creator), Err(Error::InvalidURI));
    }

    #[test]
    fn blacklisted_wallet_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Blacklisted(creator.clone()), &true);
        let uri = String::from_str(&env, "ipfs://QmTest");
        assert_eq!(validate_mint(&env, 1, &uri, &creator), Err(Error::Unauthorized));
    }
}
