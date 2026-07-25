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
///
/// Optimized: performs a single storage read (load-check-append) instead of
/// the previous two reads (contains-check + load-for-append).
pub fn add_token_to_wallet(
    env: &Env,
    wallet: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    let mut tokens = get_wallet_tokens(env, wallet);
    if tokens.iter().any(|t| t == token_id) {
        return Err(Error::DuplicateWalletEntry);
    }
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

/// Append `token_id` to a caller-managed in-memory wallet index without
/// performing any I/O.  The caller is responsible for calling
/// [`flush_wallet_cache`] once all batch additions are complete.
///
/// This helper exists so that a batch mint sharing a single owner can
/// accumulate token IDs in memory with a single final persistent write,
/// instead of issuing `get_wallet_tokens` + write for every NFT in the batch.
///
/// # Errors
/// Returns [`Error::DuplicateWalletEntry`] if `token_id` is already present in
/// `cache` (same semantics as [`add_token_to_wallet`]).
///
/// Savings per N-item same-owner batch: −(N−1) persistent reads, −(N−1)
/// persistent writes of the wallet index vector.
pub fn add_token_to_wallet_in_memory(
    cache: &mut Vec<TokenId>,
    token_id: TokenId,
) -> Result<(), Error> {
    if cache.iter().any(|t| t == token_id) {
        return Err(Error::DuplicateWalletEntry);
    }
    cache.push_back(token_id);
    Ok(())
}

/// Persist a wallet index `cache` (previously populated via
/// [`add_token_to_wallet_in_memory`]) to storage.
pub fn flush_wallet_cache(env: &Env, wallet: &Address, cache: &Vec<TokenId>) {
    env.storage()
        .persistent()
        .set(&DataKey::WalletTokens(wallet.clone()), cache);
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
