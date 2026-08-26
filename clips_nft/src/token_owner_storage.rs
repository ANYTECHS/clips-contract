//! Token owner storage — dedicated ownership record for every NFT.
//!
//! Stores a single `Address` per token under its own storage key, keeping
//! ownership reads and writes independent of the full `TokenData` record.
//! This satisfies issue #505: save owner, retrieve owner, update owner.
//!
//! # Storage
//! Key: `DataKey::TokenOwner(token_id)` → `Address` (persistent)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenData, TokenId};

// ── Write ─────────────────────────────────────────────────────────────────────

/// Persist `owner` as the owner of `token_id`.
///
/// Overwrites any previously stored value — use this both for initial
/// assignment on mint and for ownership transfers.
pub fn save_owner(env: &Env, token_id: TokenId, owner: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::TokenOwner(token_id), owner);
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Return the owner of `token_id`.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] when no ownership record exists for
/// `token_id`.
pub fn get_owner(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenOwner(token_id))
        .ok_or(Error::TokenNotFound)
}

// ── Update ────────────────────────────────────────────────────────────────────

/// Transfer ownership of `token_id` to `new_owner`.
///
/// Verifies the record exists before writing so callers get a clear
/// [`Error::TokenNotFound`] on attempts to update a non-existent token
/// rather than silently creating a stale entry.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if `token_id` has no ownership record.
pub fn update_owner(env: &Env, token_id: TokenId, new_owner: &Address) -> Result<(), Error> {
    // Confirm the token exists before overwriting.
    if !env
        .storage()
        .persistent()
        .has(&DataKey::TokenOwner(token_id))
    {
        return Err(Error::TokenNotFound);
    }
    save_owner(env, token_id, new_owner);
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return `true` if an ownership record exists for `token_id`.
pub fn has_owner(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::TokenOwner(token_id))
}

/// Verify that `expected_owner` is the current owner of `token_id`.
///
/// # Errors
/// Returns [`Error::Unauthorized`] if `expected_owner` does not match the stored owner.
pub fn verify_owner(env: &Env, token_id: TokenId, expected_owner: &Address) -> Result<(), Error> {
    let owner = get_owner(env, token_id)?;
    if owner != *expected_owner {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Remove the ownership record for `token_id` (used during mint rollback).
pub fn remove_owner(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::TokenOwner(token_id));
    // Also remove the legacy Token entry kept in sync by assign_owner.
    env.storage()
        .persistent()
        .remove(&DataKey::Token(token_id));
}

// ── Compatibility helpers used by atomic_mint / mint_validator ────────────────

/// Reject owners that are the contract's own address (cannot self-own NFTs).
pub fn validate_owner(env: &Env, owner: &Address) -> Result<(), Error> {
    if *owner == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate and save the owner for a new mint.
///
/// Wraps [`validate_owner`] + [`save_owner`] in one call so `atomic_mint`
/// can mirror the previous `assign_owner` signature without depending on
/// the old `TokenData` struct.
pub fn assign_owner(
    env: &Env,
    token_id: TokenId,
    owner: &Address,
    clip_id: u32,
) -> Result<(), Error> {
    validate_owner(env, owner)?;
    // Write the dedicated ownership key (issue #505).
    save_owner(env, token_id, owner);
    // Also write the legacy TokenData entry so token_exists() / token_storage
    // callers that inspect DataKey::Token remain consistent.
    env.storage().persistent().set(
        &DataKey::Token(token_id),
        &TokenData {
            owner: owner.clone(),
            clip_id,
        },
    );
    Ok(())
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

    // ── save_owner ────────────────────────────────────────────────────────────

    #[test]
    fn save_owner_persists_address() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 1, &owner);
            assert_eq!(get_owner(env, 1).unwrap(), owner);
        });
    }

    #[test]
    fn save_owner_overwrites_existing_record() {
        with_contract(|env| {
            let first = Address::generate(env);
            let second = Address::generate(env);
            save_owner(env, 1, &first);
            save_owner(env, 1, &second);
            assert_eq!(get_owner(env, 1).unwrap(), second);
        });
    }

    #[test]
    fn save_owner_different_tokens_are_independent() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            save_owner(env, 1, &owner_a);
            save_owner(env, 2, &owner_b);
            assert_eq!(get_owner(env, 1).unwrap(), owner_a);
            assert_eq!(get_owner(env, 2).unwrap(), owner_b);
        });
    }

    // ── get_owner ─────────────────────────────────────────────────────────────

    #[test]
    fn get_owner_returns_token_not_found_when_absent() {
        with_contract(|env| {
            assert_eq!(get_owner(env, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn get_owner_returns_correct_address_after_save() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 5, &owner);
            assert_eq!(get_owner(env, 5).unwrap(), owner);
        });
    }

    // ── update_owner ──────────────────────────────────────────────────────────

    #[test]
    fn update_owner_replaces_existing_record() {
        with_contract(|env| {
            let original = Address::generate(env);
            let updated = Address::generate(env);
            save_owner(env, 3, &original);
            update_owner(env, 3, &updated).unwrap();
            assert_eq!(get_owner(env, 3).unwrap(), updated);
        });
    }

    #[test]
    fn update_owner_fails_when_token_not_found() {
        with_contract(|env| {
            let new_owner = Address::generate(env);
            assert_eq!(update_owner(env, 42, &new_owner), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn update_owner_does_not_affect_other_tokens() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let new_owner = Address::generate(env);
            save_owner(env, 1, &owner_a);
            save_owner(env, 2, &owner_b);
            update_owner(env, 1, &new_owner).unwrap();
            assert_eq!(get_owner(env, 1).unwrap(), new_owner);
            assert_eq!(get_owner(env, 2).unwrap(), owner_b); // untouched
        });
    }

    // ── has_owner / remove_owner ───────────────────────────────────────────────

    #[test]
    fn has_owner_returns_false_when_absent() {
        with_contract(|env| {
            assert!(!has_owner(env, 10));
        });
    }

    #[test]
    fn has_owner_returns_true_after_save() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 10, &owner);
            assert!(has_owner(env, 10));
        });
    }

    #[test]
    fn remove_owner_clears_record() {
        with_contract(|env| {
            let owner = Address::generate(env);
            save_owner(env, 7, &owner);
            remove_owner(env, 7);
            assert!(!has_owner(env, 7));
            assert_eq!(get_owner(env, 7), Err(Error::TokenNotFound));
        });
    }
}
