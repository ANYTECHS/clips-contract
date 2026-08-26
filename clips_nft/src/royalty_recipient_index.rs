//! Royalty recipient index — NFT index per royalty recipient (issue #785).
//!
//! Maintains a reverse index mapping each royalty recipient address to the
//! list of token IDs whose royalty points to that recipient.  This lets
//! marketplaces and indexers answer "which NFTs does this wallet earn
//! royalties from?" in a single on-chain read.
//!
//! # Storage
//! Key: `DataKey::RecipientTokens(recipient)` → `Vec<TokenId>` (persistent storage)
//!
//! # Acceptance criteria
//! - **Add token to recipient index** — [`add_token_to_recipient`]
//! - **Remove token from recipient index** — [`remove_token_from_recipient`]
//! - **Query recipient NFTs** — [`get_recipient_tokens`]
//! - **Prevent duplicate entries** — returns [`Error::DuplicateRecord`] on
//!   repeated add of the same token ID for the same recipient.

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, Error, TokenId};

// ─── Public API ───────────────────────────────────────────────────────────────

/// Return `true` if `token_id` is already in `recipient`'s index.
pub fn recipient_contains_token(env: &Env, recipient: &Address, token_id: TokenId) -> bool {
    get_recipient_tokens(env, recipient)
        .iter()
        .any(|t| t == token_id)
}

/// Add `token_id` to the index for `recipient`.
///
/// Insertion order is preserved.
///
/// # Errors
/// Returns [`Error::DuplicateRecord`] if `token_id` is already indexed for
/// this recipient — prevents the same NFT appearing more than once in the
/// same recipient's list.
pub fn add_token_to_recipient(
    env: &Env,
    recipient: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    let mut tokens = get_recipient_tokens(env, recipient);
    if tokens.iter().any(|t| t == token_id) {
        return Err(Error::DuplicateRecord);
    }
    tokens.push_back(token_id);
    env.storage()
        .persistent()
        .set(&DataKey::RecipientTokens(recipient.clone()), &tokens);
    Ok(())
}

/// Remove `token_id` from `recipient`'s index.
///
/// Rebuilds the stored `Vec` without the entry.  A no-op if the token is
/// not present (nothing to remove → no error).
pub fn remove_token_from_recipient(env: &Env, recipient: &Address, token_id: TokenId) {
    let existing = get_recipient_tokens(env, recipient);
    let mut updated: Vec<TokenId> = Vec::new(env);
    for t in existing.iter() {
        if t != token_id {
            updated.push_back(t);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::RecipientTokens(recipient.clone()), &updated);
}

/// Return all token IDs indexed under `recipient`.
///
/// Returns an empty [`Vec`] if no NFTs are recorded for this address.
pub fn get_recipient_tokens(env: &Env, recipient: &Address) -> Vec<TokenId> {
    env.storage()
        .persistent()
        .get(&DataKey::RecipientTokens(recipient.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Return the number of tokens indexed for `recipient`.
pub fn recipient_token_count(env: &Env, recipient: &Address) -> u32 {
    get_recipient_tokens(env, recipient).len()
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

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

    // ── add_token_to_recipient ────────────────────────────────────────────────

    #[test]
    fn add_single_token_to_recipient() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 1).unwrap();
            let tokens = get_recipient_tokens(env, &recipient);
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens.get(0).unwrap(), 1);
        });
    }

    #[test]
    fn add_multiple_tokens_preserves_order() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 10).unwrap();
            add_token_to_recipient(env, &recipient, 20).unwrap();
            add_token_to_recipient(env, &recipient, 30).unwrap();

            let tokens = get_recipient_tokens(env, &recipient);
            assert_eq!(tokens.len(), 3);
            assert_eq!(tokens.get(0).unwrap(), 10);
            assert_eq!(tokens.get(1).unwrap(), 20);
            assert_eq!(tokens.get(2).unwrap(), 30);
        });
    }

    #[test]
    fn add_duplicate_token_returns_duplicate_record() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 5).unwrap();
            let err = add_token_to_recipient(env, &recipient, 5)
                .expect_err("second add of same token should fail");
            assert_eq!(err, Error::DuplicateRecord);
            // The list must still contain only one entry.
            assert_eq!(get_recipient_tokens(env, &recipient).len(), 1);
        });
    }

    // ── remove_token_from_recipient ───────────────────────────────────────────

    #[test]
    fn remove_existing_token_reduces_list() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 1).unwrap();
            add_token_to_recipient(env, &recipient, 2).unwrap();
            add_token_to_recipient(env, &recipient, 3).unwrap();

            remove_token_from_recipient(env, &recipient, 2);

            let tokens = get_recipient_tokens(env, &recipient);
            assert_eq!(tokens.len(), 2);
            assert_eq!(tokens.get(0).unwrap(), 1);
            assert_eq!(tokens.get(1).unwrap(), 3);
        });
    }

    #[test]
    fn remove_last_token_leaves_empty_list() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 42).unwrap();
            remove_token_from_recipient(env, &recipient, 42);
            assert_eq!(get_recipient_tokens(env, &recipient).len(), 0);
        });
    }

    #[test]
    fn remove_nonexistent_token_is_noop() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 1).unwrap();
            // removing a token that was never added should not panic or error
            remove_token_from_recipient(env, &recipient, 99);
            assert_eq!(get_recipient_tokens(env, &recipient).len(), 1);
        });
    }

    #[test]
    fn remove_from_empty_index_is_noop() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            // No panic expected; the list remains empty.
            remove_token_from_recipient(env, &recipient, 7);
            assert_eq!(get_recipient_tokens(env, &recipient).len(), 0);
        });
    }

    // ── get_recipient_tokens ──────────────────────────────────────────────────

    #[test]
    fn get_recipient_tokens_empty_for_unknown_address() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let tokens = get_recipient_tokens(env, &recipient);
            assert_eq!(tokens.len(), 0);
        });
    }

    // ── recipient_contains_token ──────────────────────────────────────────────

    #[test]
    fn recipient_contains_token_true_after_add() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 11).unwrap();
            assert!(recipient_contains_token(env, &recipient, 11));
        });
    }

    #[test]
    fn recipient_contains_token_false_before_add() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert!(!recipient_contains_token(env, &recipient, 11));
        });
    }

    #[test]
    fn recipient_contains_token_false_after_remove() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 11).unwrap();
            remove_token_from_recipient(env, &recipient, 11);
            assert!(!recipient_contains_token(env, &recipient, 11));
        });
    }

    // ── recipient_token_count ─────────────────────────────────────────────────

    #[test]
    fn count_increases_on_add() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert_eq!(recipient_token_count(env, &recipient), 0);
            add_token_to_recipient(env, &recipient, 1).unwrap();
            assert_eq!(recipient_token_count(env, &recipient), 1);
            add_token_to_recipient(env, &recipient, 2).unwrap();
            assert_eq!(recipient_token_count(env, &recipient), 2);
        });
    }

    #[test]
    fn count_decreases_on_remove() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 1).unwrap();
            add_token_to_recipient(env, &recipient, 2).unwrap();
            remove_token_from_recipient(env, &recipient, 1);
            assert_eq!(recipient_token_count(env, &recipient), 1);
        });
    }

    // ── Index isolation per recipient ─────────────────────────────────────────

    #[test]
    fn indexes_are_isolated_per_recipient() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);

            add_token_to_recipient(env, &alice, 1).unwrap();
            add_token_to_recipient(env, &alice, 2).unwrap();
            add_token_to_recipient(env, &bob, 3).unwrap();

            assert_eq!(get_recipient_tokens(env, &alice).len(), 2);
            assert_eq!(get_recipient_tokens(env, &bob).len(), 1);

            assert!(recipient_contains_token(env, &alice, 1));
            assert!(recipient_contains_token(env, &alice, 2));
            assert!(!recipient_contains_token(env, &alice, 3));
            assert!(recipient_contains_token(env, &bob, 3));
            assert!(!recipient_contains_token(env, &bob, 1));
        });
    }

    #[test]
    fn same_token_can_be_in_multiple_recipient_indexes() {
        with_contract(|env| {
            // Token 1 can appear in both alice's and bob's indexes (different royalty
            // configs or a multi-recipient scenario is handled at the caller level).
            let alice = Address::generate(env);
            let bob = Address::generate(env);

            add_token_to_recipient(env, &alice, 1).unwrap();
            add_token_to_recipient(env, &bob, 1).unwrap();

            assert!(recipient_contains_token(env, &alice, 1));
            assert!(recipient_contains_token(env, &bob, 1));
        });
    }

    // ── add-remove-re-add cycle ───────────────────────────────────────────────

    #[test]
    fn token_can_be_re_added_after_removal() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            add_token_to_recipient(env, &recipient, 5).unwrap();
            remove_token_from_recipient(env, &recipient, 5);
            // Should be addable again without a DuplicateRecord error.
            add_token_to_recipient(env, &recipient, 5).unwrap();
            assert_eq!(recipient_token_count(env, &recipient), 1);
        });
    }
}
