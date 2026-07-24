//! Mint validator — validates mint requests before NFT creation.
//!
//! Resolves issue #429. Checks:
//! - Duplicate clip (clip already minted)
//! - Metadata URI is valid (supported protocol, correct length)
//! - Creator address is valid
//! - Wallet is not blacklisted
//! - Royalty configuration is valid (percentage, recipient, max limit)

use soroban_sdk::{Address, Env, String};

use crate::types::{DataKey, Error, Royalty};
use crate::metadata::validation::validate_metadata_uri;
use crate::royalty_validator::validate_royalty;

/// Validate a mint request before any state is written.
///
/// # Checks (in order)
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
