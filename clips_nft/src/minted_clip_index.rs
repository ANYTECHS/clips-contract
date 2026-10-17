//! Minted clip index — tracks which ClipCash clip IDs have been minted as NFTs.
//!
//! Provides an existence check and enforces that the same clip ID cannot be minted twice.
//!
//! # Storage
//! Key: `DataKey::ClipMinted(clip_id)` → bool (persistent)

use soroban_sdk::Env;

use crate::types::{DataKey, Error};

/// Register `clip_id` in the minted-clip index.
///
/// Returns `Err(ClipAlreadyMinted)` if `clip_id` is already present.
pub fn add_clip(env: &Env, clip_id: u32) -> Result<(), Error> {
    if env
        .storage()
        .persistent()
        .has(&DataKey::ClipMinted(clip_id))
    {
        return Err(Error::ClipAlreadyMinted);
    }
    env.storage()
        .persistent()
        .set(&DataKey::ClipMinted(clip_id), &true);
    Ok(())
}

/// Return `true` if `clip_id` has been added to the minted-clip index.
pub fn clip_exists(env: &Env, clip_id: u32) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::ClipMinted(clip_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn add_and_check_clip_exists() {
        with_contract(|env| {
            assert!(!clip_exists(env, 42));
            add_clip(env, 42).unwrap();
            assert!(clip_exists(env, 42));
        });
    }

    #[test]
    fn add_duplicate_clip_fails() {
        with_contract(|env| {
            add_clip(env, 7).unwrap();
            assert_eq!(add_clip(env, 7), Err(Error::ClipAlreadyMinted));
        });
    }
}
