//! Virality score metadata — store and retrieve AI-generated virality scores.
//!
//! Score range: 0–100 (inclusive). Values outside this range are rejected.

use soroban_sdk::Env;

use crate::types::{DataKey, Error, TokenId};

const MAX_SCORE: u32 = 100;

/// Persist the virality score for `token_id`. Score must be 0–100.
pub fn set_virality_score(env: &Env, token_id: TokenId, score: u32) -> Result<(), Error> {
    if score > MAX_SCORE {
        return Err(Error::InvalidBasisPoints);
    }
    env.storage()
        .persistent()
        .set(&DataKey::ViralityScore(token_id), &score);
    Ok(())
}

/// Return the virality score for `token_id`, or `Err(TokenNotFound)` if not set.
pub fn get_virality_score(env: &Env, token_id: TokenId) -> Result<u32, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::ViralityScore(token_id))
        .ok_or(Error::TokenNotFound)
}
