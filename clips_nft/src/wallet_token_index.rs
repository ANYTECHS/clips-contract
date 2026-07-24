//! Wallet token index — maintains a token ownership index for every wallet.
//!
//! # Storage
//! Key: `DataKey::WalletTokens(wallet)` → `Vec<TokenId>` (persistent)

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, Error, TokenId};

/// Return `true` if `token_id` is already indexed for `wallet`.
pub fn wallet_contains_token(env: &Env, wallet: &Address, token_id: TokenId) -> bool {
    get_wallet_tokens(env, wallet).iter().any(|t| t == token_id)
}

/// Add a token to a wallet's ownership index.
///
/// Returns `Err(DuplicateWalletEntry)` if the token is already indexed.
pub fn add_token_to_wallet(
    env: &Env,
    wallet: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    if wallet_contains_token(env, wallet, token_id) {
        return Err(Error::DuplicateWalletEntry);
    }
    let mut tokens = get_wallet_tokens(env, wallet);
    tokens.push_back(token_id);
    env.storage()
        .persistent()
        .set(&DataKey::WalletTokens(wallet.clone()), &tokens);
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn adds_token_to_wallet_index() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            add_token_to_wallet(env, &wallet, 1).unwrap();
            let tokens = get_wallet_tokens(env, &wallet);
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens.get(0).unwrap(), 1);
        });
    }

    #[test]
    fn supports_multiple_nfts_per_wallet() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            add_token_to_wallet(env, &wallet, 1).unwrap();
            add_token_to_wallet(env, &wallet, 2).unwrap();
            add_token_to_wallet(env, &wallet, 3).unwrap();

            let tokens = get_wallet_tokens(env, &wallet);
            assert_eq!(tokens.len(), 3);
            assert_eq!(tokens.get(0).unwrap(), 1);
            assert_eq!(tokens.get(1).unwrap(), 2);
            assert_eq!(tokens.get(2).unwrap(), 3);
        });
    }

    #[test]
    fn prevents_duplicate_entries() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            add_token_to_wallet(env, &wallet, 5).unwrap();
            assert_eq!(
                add_token_to_wallet(env, &wallet, 5),
                Err(Error::DuplicateWalletEntry)
            );
            assert_eq!(get_wallet_tokens(env, &wallet).len(), 1);
        });
    }

    #[test]
    fn remove_token_from_wallet_works() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            add_token_to_wallet(env, &wallet, 1).unwrap();
            add_token_to_wallet(env, &wallet, 2).unwrap();
            remove_token_from_wallet(env, &wallet, 1);

            let tokens = get_wallet_tokens(env, &wallet);
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens.get(0).unwrap(), 2);
        });
    }
}
