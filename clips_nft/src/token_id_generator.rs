
//! Token ID Generator — generates unique sequential token IDs for ClipCash NFTs.
//!
//! Guarantees uniqueness and consistency by storing the next available ID
//! and atomically incrementing it on each generation.
//!
//! # Storage
//! Key: `StorageKey::NextTokenId` (instance storage)

use soroban_sdk::Env;

use crate::storage::keys::StorageKey;
use crate::types::{Error, TokenId};

/// Get the next available token ID without consuming it.
pub fn peek_next_token_id(env: &Env) -> TokenId {
    env.storage()
        .instance()
        .get(&StorageKey::NextTokenId)
        .unwrap_or(1)
}

/// Set the next available token ID to a specific value (for initialization/migration).
pub fn save_next_token_id(env: &Env, next_id: TokenId) {
    env.storage()
        .instance()
        .set(&StorageKey::NextTokenId, &next_id);
}

/// Generate and return a new unique token ID, incrementing the stored counter.
///
/// # Errors
/// Returns [`Error::InvalidLimit`] if incrementing the counter would overflow.
pub fn generate_token_id(env: &Env) -> Result<TokenId, Error> {
    let current_id = peek_next_token_id(env);
    let next_id = current_id.checked_add(1).ok_or(Error::InvalidLimit)?;
    env.storage()
        .instance()
        .set(&StorageKey::NextTokenId, &next_id);
    Ok(current_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn peek_next_token_id_defaults_to_1() {
        let env = Env::default();
        assert_eq!(peek_next_token_id(&env), 1);
    }

    #[test]
    fn save_and_peek_next_token_id() {
        let env = Env::default();
        save_next_token_id(&env, 100);
        assert_eq!(peek_next_token_id(&env), 100);
    }

    #[test]
    fn generate_token_id_returns_1_first() {
        let env = Env::default();
        let token_id = generate_token_id(&env).unwrap();
        assert_eq!(token_id, 1);
    }

    #[test]
    fn generate_token_id_increments_each_time() {
        let env = Env::default();
        assert_eq!(generate_token_id(&env).unwrap(), 1);
        assert_eq!(generate_token_id(&env).unwrap(), 2);
        assert_eq!(generate_token_id(&env).unwrap(), 3);
    }

    #[test]
    fn peek_next_token_id_returns_current_id_before_generation() {
        let env = Env::default();
        assert_eq!(peek_next_token_id(&env), 1);
        generate_token_id(&env).unwrap();
        assert_eq!(peek_next_token_id(&env), 2);
    }

    #[test]
    fn generate_token_id_handles_overflow() {
        let env = Env::default();
        // Set next token ID to u32::MAX
        save_next_token_id(&env, u32::MAX);
        // Attempt to generate should fail with InvalidLimit
        assert_eq!(generate_token_id(&env), Err(Error::InvalidLimit));
    }
}
