//! Creator portfolio index (issue #674).
//!
//! Maintains an index of the NFTs created by each creator so a creator's full
//! portfolio can be queried on-chain. The index preserves insertion order and
//! rejects duplicate registrations.
//!
//! # Storage
//! Key: `DataKey::CreatorTokens(creator)` → `Vec<TokenId>` (persistent storage)

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, Error, TokenId};

/// Return `true` if `token_id` is already indexed for `creator`.
pub fn creator_contains_token(env: &Env, creator: &Address, token_id: TokenId) -> bool {
    get_creator_portfolio(env, creator)
        .iter()
        .any(|t| t == token_id)
}

/// Add `token_id` to `creator`'s portfolio index.
///
/// # Errors
/// Returns [`Error::DuplicateRecord`] if the token is already indexed for this creator.
///
/// Optimized: performs a single storage read (load-check-append) instead of
/// the previous two reads (contains-check + load-for-append).
pub fn add_token_to_creator(
    env: &Env,
    creator: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    let mut tokens = get_creator_portfolio(env, creator);
    if tokens.iter().any(|t| t == token_id) {
        return Err(Error::DuplicateRecord);
    }
    tokens.push_back(token_id);
    env.storage()
        .persistent()
        .set(&DataKey::CreatorTokens(creator.clone()), &tokens);
    Ok(())
}

/// Retrieve every token ID created by `creator`. Returns an empty vec if none recorded.
pub fn get_creator_portfolio(env: &Env, creator: &Address) -> Vec<TokenId> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorTokens(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Append `token_id` to a caller-managed in-memory creator portfolio without
/// performing any storage I/O.  Call [`flush_creator_portfolio_cache`] once
/// all batch additions are complete to persist the final state.
///
/// Use this helper during batch mints where many NFTs share the same creator
/// to avoid N redundant `get_creator_portfolio`/write calls per batch.
///
/// # Errors
/// Returns [`Error::DuplicateRecord`] if `token_id` already exists in `cache`.
///
/// Savings per N-item same-creator batch: −(N−1) persistent reads, −(N−1)
/// persistent writes of the creator portfolio vector.
pub fn add_token_to_creator_in_memory(
    cache: &mut Vec<TokenId>,
    token_id: TokenId,
) -> Result<(), Error> {
    if cache.iter().any(|t| t == token_id) {
        return Err(Error::DuplicateRecord);
    }
    cache.push_back(token_id);
    Ok(())
}

/// Persist a creator portfolio cache (populated via
/// [`add_token_to_creator_in_memory`]) to persistent storage.
pub fn flush_creator_portfolio_cache(env: &Env, creator: &Address, cache: &Vec<TokenId>) {
    env.storage()
        .persistent()
        .set(&DataKey::CreatorTokens(creator.clone()), cache);
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
    fn adds_token_to_creator_portfolio() {
        with_contract(|env| {
            let creator = Address::generate(env);
            add_token_to_creator(env, &creator, 1).unwrap();
            let portfolio = get_creator_portfolio(env, &creator);
            assert_eq!(portfolio.len(), 1);
            assert_eq!(portfolio.get(0).unwrap(), 1);
        });
    }

    #[test]
    fn supports_multiple_nfts_per_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            add_token_to_creator(env, &creator, 10).unwrap();
            add_token_to_creator(env, &creator, 20).unwrap();
            add_token_to_creator(env, &creator, 30).unwrap();

            let portfolio = get_creator_portfolio(env, &creator);
            assert_eq!(portfolio.len(), 3);
            assert_eq!(portfolio.get(0).unwrap(), 10);
            assert_eq!(portfolio.get(1).unwrap(), 20);
            assert_eq!(portfolio.get(2).unwrap(), 30);
        });
    }

    #[test]
    fn prevents_duplicate_entries() {
        with_contract(|env| {
            let creator = Address::generate(env);
            add_token_to_creator(env, &creator, 5).unwrap();
            assert_eq!(
                add_token_to_creator(env, &creator, 5),
                Err(Error::DuplicateRecord)
            );
            assert_eq!(get_creator_portfolio(env, &creator).len(), 1);
        });
    }

    #[test]
    fn portfolios_are_isolated_per_creator() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            add_token_to_creator(env, &alice, 1).unwrap();
            add_token_to_creator(env, &bob, 2).unwrap();

            assert_eq!(get_creator_portfolio(env, &alice).len(), 1);
            assert_eq!(get_creator_portfolio(env, &bob).len(), 1);
            assert!(creator_contains_token(env, &alice, 1));
            assert!(!creator_contains_token(env, &alice, 2));
        });
    }

    #[test]
    fn empty_portfolio_for_unknown_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert_eq!(get_creator_portfolio(env, &creator).len(), 0);
        });
    }
}
