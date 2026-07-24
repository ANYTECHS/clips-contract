//! Creator storage — records the original creator wallet for every NFT.
//!
//! Resolves issue #665: [Minting] Assign NFT Creator During Mint.
//!
//! The creator is assigned once at mint time and persisted for the lifetime
//! of the token.  It is used for attribution and as the default royalty
//! recipient when no explicit override is provided.
//!
//! # Storage
//! Key: `DataKey::Creator(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::creator_event;
use crate::types::{DataKey, Error, TokenId};

/// Validate that the creator address is structurally present.
///
/// Soroban `Address` values are always non-null at the type level, so this
/// function acts as an explicit gate that can be extended with additional
/// business rules (e.g. blacklist checks) in the future.
///
/// # Errors
/// - [`Error::EmptyCreator`] — returned when the caller explicitly signals an
///   absent creator via a sentinel pattern (reserved for future use).
pub fn validate_creator(_creator: &Address) -> Result<(), Error> {
    // The Soroban type system guarantees that an `Address` is non-empty.
    // This function is a deliberate hook so that stricter checks
    // (e.g. blacklist, admin guard) can be added without changing call sites.
    Ok(())
}

/// Validate and persist the creator wallet for a token.
///
/// Should be called once per mint, before the mint is finalised.
///
/// # Errors
/// - [`Error::EmptyCreator`] — propagated from [`validate_creator`].
pub fn set_creator(env: &Env, token_id: TokenId, creator: &Address) -> Result<(), Error> {
    validate_creator(creator)?;
    env.storage()
        .persistent()
        .set(&DataKey::Creator(token_id), creator);
    Ok(())
}

/// Assign a creator to a newly minted NFT and emit [`CreatorAssignedEvent`].
///
/// # Arguments
/// * `token_id` — Newly minted token.
/// * `creator`  — Creator wallet.
/// * `clip_id`  — Linked off-chain clip identifier.
pub fn assign_creator(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
    clip_id: u32,
) -> Result<(), Error> {
    set_creator(env, token_id, creator);
    let timestamp = env.ledger().timestamp();
    creator_event::emit_creator_assigned(env, token_id, creator, clip_id, timestamp);
    Ok(())
}

/// Read the creator wallet for a token.
///
/// Returns `Err(TokenNotFound)` if no creator has been recorded for this
/// token (i.e. the token was minted before this feature was introduced, or
/// the token does not exist).
pub fn get_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Creator(token_id))
        .ok_or(Error::TokenNotFound)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn set_and_get_creator() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let token_id = 1u32;

        set_creator(&env, token_id, &creator).expect("valid creator should be stored");
        assert_eq!(get_creator(&env, token_id), Ok(creator));
    }

    #[test]
    fn creator_is_scoped_per_token() {
        let env = Env::default();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        set_creator(&env, 1, &creator_a).unwrap();
        set_creator(&env, 2, &creator_b).unwrap();

        assert_eq!(get_creator(&env, 1), Ok(creator_a));
        assert_eq!(get_creator(&env, 2), Ok(creator_b));
    }

    #[test]
    fn get_creator_returns_not_found_when_absent() {
        let env = Env::default();
        assert_eq!(get_creator(&env, 99), Err(Error::TokenNotFound));
    }

    #[test]
    fn creator_persists_after_reassignment() {
        let env = Env::default();
        let token_id = 5u32;
        let creator_v1 = Address::generate(&env);
        let creator_v2 = Address::generate(&env);

        set_creator(&env, token_id, &creator_v1).unwrap();
        // Overwrite (e.g. migration scenario)
        set_creator(&env, token_id, &creator_v2).unwrap();

        assert_eq!(get_creator(&env, token_id), Ok(creator_v2));
    }
}
