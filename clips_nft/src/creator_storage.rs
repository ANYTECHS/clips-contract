//! Creator storage — records the creator wallet for every NFT.
//!
//! # Storage
//! Key: `DataKey::Creator(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

/// Save the creator wallet for a token.
pub fn set_creator(env: &Env, token_id: TokenId, creator: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Creator(token_id), creator);
}

/// Read the creator wallet for a token.
///
/// Returns `Err(TokenNotFound)` if no creator has been recorded.
pub fn get_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Creator(token_id))
        .ok_or(Error::TokenNotFound)
}
