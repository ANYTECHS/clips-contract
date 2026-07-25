//! Owner portfolio index (issue #675).
//!
//! Updates the owner's NFT portfolio after a successful mint. Tokens are stored
//! in insertion order (ordering is preserved) and duplicate entries are rejected.
//!
//! # Storage
//! Key: `DataKey::OwnerTokens(owner)` → `Vec<TokenId>` (persistent storage)

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, Error, TokenId};

/// Return `true` if `token_id` is already indexed for `owner`.
pub fn owner_contains_token(env: &Env, owner: &Address, token_id: TokenId) -> bool {
    get_owner_portfolio(env, owner).iter().any(|t| t == token_id)
}

/// Add `token_id` to `owner`'s portfolio index, appended at the end to preserve order.
///
/// # Errors
/// Returns [`Error::DuplicateRecord`] if the token is already indexed for this owner.
///
/// Optimization: performs a single storage read (load-check-append) instead of
/// the previous two reads (`owner_contains_token` first, then `get_owner_portfolio`
/// for append).  Both reads returned the same `DataKey::OwnerTokens(owner)`
/// payload; loading once and then scanning in memory is strictly cheaper.
///
/// Savings: −1 persistent read per call.
pub fn add_token_to_owner(env: &Env, owner: &Address, token_id: TokenId) -> Result<(), Error> {
    let mut tokens = get_owner_portfolio(env, owner);
    if tokens.iter().any(|t| t == token_id) {
        return Err(Error::DuplicateRecord);
    }
    tokens.push_back(token_id);
    env.storage()
        .persistent()
        .set(&DataKey::OwnerTokens(owner.clone()), &tokens);
    Ok(())
}

/// Retrieve every token ID owned by `owner`, in insertion order. Empty if none recorded.
pub fn get_owner_portfolio(env: &Env, owner: &Address) -> Vec<TokenId> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerTokens(owner.clone()))
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
    fn adds_token_to_owner_portfolio() {
        with_contract(|env| {
            let owner = Address::generate(env);
            add_token_to_owner(env, &owner, 1).unwrap();
            let portfolio = get_owner_portfolio(env, &owner);
            assert_eq!(portfolio.len(), 1);
            assert_eq!(portfolio.get(0).unwrap(), 1);
        });
    }

    #[test]
    fn preserves_insertion_order() {
        with_contract(|env| {
            let owner = Address::generate(env);
            add_token_to_owner(env, &owner, 30).unwrap();
            add_token_to_owner(env, &owner, 10).unwrap();
            add_token_to_owner(env, &owner, 20).unwrap();

            let portfolio = get_owner_portfolio(env, &owner);
            assert_eq!(portfolio.len(), 3);
            assert_eq!(portfolio.get(0).unwrap(), 30);
            assert_eq!(portfolio.get(1).unwrap(), 10);
            assert_eq!(portfolio.get(2).unwrap(), 20);
        });
    }

    #[test]
    fn prevents_duplicate_entries() {
        with_contract(|env| {
            let owner = Address::generate(env);
            add_token_to_owner(env, &owner, 7).unwrap();
            assert_eq!(
                add_token_to_owner(env, &owner, 7),
                Err(Error::DuplicateRecord)
            );
            assert_eq!(get_owner_portfolio(env, &owner).len(), 1);
        });
    }

    #[test]
    fn portfolios_are_isolated_per_owner() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            add_token_to_owner(env, &alice, 1).unwrap();
            add_token_to_owner(env, &bob, 2).unwrap();

            assert!(owner_contains_token(env, &alice, 1));
            assert!(!owner_contains_token(env, &alice, 2));
            assert_eq!(get_owner_portfolio(env, &bob).get(0).unwrap(), 2);
        });
    }

    #[test]
    fn empty_portfolio_for_unknown_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(get_owner_portfolio(env, &owner).len(), 0);
        });
    }
}
