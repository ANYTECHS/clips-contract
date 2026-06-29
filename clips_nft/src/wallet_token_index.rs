//! Wallet token index — maintains a token ownership index for every wallet.
//!
//! # Storage
//! Key: `DataKey::WalletTokens(wallet)` → `Vec<TokenId>` (persistent storage)

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, TokenId};

/// Add a token to a wallet's ownership index.
pub fn add_token_to_wallet(env: &Env, wallet: &Address, token_id: TokenId) {
    let mut tokens = get_wallet_tokens(env, wallet);
    tokens.push_back(token_id);
    env.storage()
        .persistent()
        .set(&DataKey::WalletTokens(wallet.clone()), &tokens);
}

/// Remove a token from a wallet's ownership index.
pub fn remove_token_from_wallet(env: &Env, wallet: &Address, token_id: TokenId) {
    let tokens = get_wallet_tokens(env, wallet);
    let mut updated: Vec<TokenId> = Vec::new(env);
    for t in tokens.iter() {
        if t != token_id {
            updated.push_back(t);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::WalletTokens(wallet.clone()), &updated);
}

/// Retrieve all token IDs owned by a wallet. Returns an empty vec if none recorded.
pub fn get_wallet_tokens(env: &Env, wallet: &Address) -> Vec<TokenId> {
    env.storage()
        .persistent()
        .get(&DataKey::WalletTokens(wallet.clone()))
        .unwrap_or_else(|| Vec::new(env))
}
