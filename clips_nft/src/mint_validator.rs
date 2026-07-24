//! Mint validator — validates mint requests before NFT creation.
//!
//! Checks clip dedup, metadata URI presence, and blacklist status.

use soroban_sdk::{Address, Env, String};

use crate::types::{DataKey, Error};

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
        with_contract(|env| {
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            assert!(validate_mint(env, 1, &uri, &owner).is_ok());
        });
    }

    #[test]
    fn duplicate_clip_fails() {
        with_contract(|env| {
            env.storage()
                .persistent()
                .set(&DataKey::ClipIdMinted(42), &0u32);
            let owner = Address::generate(env);
            let uri = String::from_str(env, "ipfs://QmTest");
            assert_eq!(
                validate_mint(env, 42, &uri, &owner),
                Err(Error::ClipAlreadyMinted)
            );
        });
    }

    #[test]
    fn empty_metadata_fails() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let uri = String::from_str(env, "");
            assert_eq!(
                validate_mint(env, 1, &uri, &owner),
                Err(Error::InvalidURI)
            );
        });
    }

    #[test]
    fn blacklisted_wallet_fails() {
        with_contract(|env| {
            let owner = Address::generate(env);
            env.storage()
                .persistent()
                .set(&DataKey::Blacklisted(owner.clone()), &true);
            let uri = String::from_str(env, "ipfs://QmTest");
            assert_eq!(
                validate_mint(env, 1, &uri, &owner),
                Err(Error::Unauthorized)
            );
        });
    }
}
