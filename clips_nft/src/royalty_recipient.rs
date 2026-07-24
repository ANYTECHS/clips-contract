//! Royalty recipient storage — maintains a direct mapping from token ID to
//! royalty recipient address.
//!
//! Resolves issue #672: [Minting] Store Royalty Recipient Mapping.
//!
//! This lightweight index complements the full [`Royalty`] struct by offering
//! O(1) recipient lookups without deserialising the complete royalty config.
//! It is written at mint time and can be updated safely by the admin.
//!
//! # Storage
//! Key: `DataKey::RoyaltyRecipient(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

/// Persist the royalty recipient for a given token.
///
/// This is a pure write — it creates or overwrites the stored address.
/// Callers are responsible for ensuring the token exists before calling
/// this function (the mint service does so as part of the atomic mint
/// pipeline).
pub fn set_royalty_recipient(env: &Env, token_id: TokenId, recipient: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyRecipient(token_id), recipient);
}

/// Return the royalty recipient for a given token.
///
/// Returns `Err(TokenNotFound)` if no recipient has been recorded.
pub fn get_royalty_recipient(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::RoyaltyRecipient(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Update the royalty recipient for an existing token, verifying the token
/// exists first.
///
/// Use this for post-mint admin updates.
///
/// # Errors
/// - [`Error::TokenNotFound`] — the token record does not exist in storage.
pub fn update_royalty_recipient(
    env: &Env,
    token_id: TokenId,
    recipient: &Address,
) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Token(token_id)) {
        return Err(Error::TokenNotFound);
    }
    set_royalty_recipient(env, token_id, recipient);
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    use crate::types::{DataKey, TokenData};

    fn store_token(env: &Env, token_id: TokenId, owner: &Address) {
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &TokenData { owner: owner.clone(), clip_id: token_id });
    }

    #[test]
    fn set_and_get_royalty_recipient() {
        let env = Env::default();
        let recipient = Address::generate(&env);
        let token_id = 1u32;

        set_royalty_recipient(&env, token_id, &recipient);
        assert_eq!(get_royalty_recipient(&env, token_id), Ok(recipient));
    }

    #[test]
    fn get_royalty_recipient_returns_not_found_when_absent() {
        let env = Env::default();
        assert_eq!(get_royalty_recipient(&env, 99u32), Err(Error::TokenNotFound));
    }

    #[test]
    fn recipient_is_scoped_per_token() {
        let env = Env::default();
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);

        set_royalty_recipient(&env, 1, &addr_a);
        set_royalty_recipient(&env, 2, &addr_b);

        assert_eq!(get_royalty_recipient(&env, 1), Ok(addr_a));
        assert_eq!(get_royalty_recipient(&env, 2), Ok(addr_b));
    }

    #[test]
    fn recipient_can_be_overwritten_by_set() {
        let env = Env::default();
        let token_id = 3u32;
        let old_addr = Address::generate(&env);
        let new_addr = Address::generate(&env);

        set_royalty_recipient(&env, token_id, &old_addr);
        set_royalty_recipient(&env, token_id, &new_addr);

        assert_eq!(get_royalty_recipient(&env, token_id), Ok(new_addr));
    }

    #[test]
    fn update_royalty_recipient_succeeds_when_token_exists() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token_id = 10u32;

        store_token(&env, token_id, &owner);
        set_royalty_recipient(&env, token_id, &owner); // initial mapping
        update_royalty_recipient(&env, token_id, &recipient)
            .expect("update should succeed for existing token");

        assert_eq!(get_royalty_recipient(&env, token_id), Ok(recipient));
    }

    #[test]
    fn update_royalty_recipient_fails_when_token_missing() {
        let env = Env::default();
        let recipient = Address::generate(&env);
        let err = update_royalty_recipient(&env, 42u32, &recipient)
            .expect_err("should fail for non-existent token");
        assert_eq!(err, Error::TokenNotFound);
    }
}
