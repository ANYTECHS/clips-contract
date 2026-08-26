//! Clip ID storage — bidirectional mapping between NFT token IDs and ClipCash clip IDs.
//!
//! Ensures every token maps to exactly one clip and prevents the same clip ID from
//! being assigned to more than one token.
//!
//! # Storage
//! - `DataKey::TokenClipId(token_id)` → clip_id (persistent)
//! - `DataKey::ClipIdMinted(clip_id)` → token_id (persistent; also used as dedup sentinel)

use soroban_sdk::Env;

use crate::types::{DataKey, Error, TokenId};

/// Associate `clip_id` with `token_id` and record the reverse mapping.
///
/// Returns `Err(ClipAlreadyMinted)` if `clip_id` is already mapped to another token.
pub fn save_clip_id(env: &Env, token_id: TokenId, clip_id: u32) -> Result<(), Error> {
    if env
        .storage()
        .persistent()
        .has(&DataKey::ClipIdMinted(clip_id))
    {
        return Err(Error::ClipAlreadyMinted);
    }
    env.storage()
        .persistent()
        .set(&DataKey::TokenClipId(token_id), &clip_id);
    env.storage()
        .persistent()
        .set(&DataKey::ClipIdMinted(clip_id), &token_id);
    Ok(())
}

/// Write the clip ID mappings without checking for duplicates.
///
/// Use only when the caller has already verified (via [`is_clip_mapped`] or
/// [`save_clip_id`] failing) that `clip_id` is unique in the same invocation,
/// such as the batch-mint path where `validate_batch_mint` already performed
/// the dedup check before any writes begin.
///
/// Saves one `has()` persistent lookup per mint in the batch path.
pub fn save_clip_id_unchecked(env: &Env, token_id: TokenId, clip_id: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::TokenClipId(token_id), &clip_id);
    env.storage()
        .persistent()
        .set(&DataKey::ClipIdMinted(clip_id), &token_id);
}

/// Return the clip ID associated with `token_id`. Returns `Err(TokenNotFound)` if absent.
pub fn get_clip_id(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenClipId(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Return `true` if `clip_id` has already been mapped to a token.
pub fn is_clip_mapped(env: &Env, clip_id: u32) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::ClipIdMinted(clip_id))
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
    fn save_and_get_clip_id() {
        with_contract(|env| {
            save_clip_id(env, 1, 42).unwrap();
            assert_eq!(get_clip_id(env, 1).unwrap(), 42);
            assert!(is_clip_mapped(env, 42));
        });
    }

    #[test]
    fn save_clip_id_duplicate_fails() {
        with_contract(|env| {
            save_clip_id(env, 1, 100).unwrap();
            let res = save_clip_id(env, 2, 100);
            assert_eq!(res, Err(Error::ClipAlreadyMinted));
        });
    }

    #[test]
    fn save_clip_id_unchecked_overwrites() {
        with_contract(|env| {
            save_clip_id_unchecked(env, 10, 200);
            assert_eq!(get_clip_id(env, 10).unwrap(), 200);
            assert!(is_clip_mapped(env, 200));
        });
    }

    #[test]
    fn get_clip_id_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_clip_id(env, 999), Err(Error::TokenNotFound));
        });
    }
}
