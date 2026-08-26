//! Creator storage — records the original creator wallet for every NFT.
//!
//! Stores a [`CreatorMetadata`] struct containing the creator wallet address,
//! optional display name, and a platform verification flag.
//!
//! Resolves issue #665: [Minting] Assign NFT Creator During Mint.
//!
//! The creator is assigned once at mint time and persisted for the lifetime
//! of the token.  It is used for attribution and as the default royalty
//! recipient when no explicit override is provided.
//!
//! # Storage
//! Key: `DataKey::Creator(token_id)` (persistent storage)

use soroban_sdk::{Address, Env, String};

use crate::creator_event;
use crate::metadata::CreatorMetadata;
use crate::types::{DataKey, Error, TokenId};

// ─── Public API ────────────────────────────────────────────────────────────────

/// Save the full creator metadata for a token.
pub fn set_creator_metadata(env: &Env, token_id: TokenId, metadata: &CreatorMetadata) {
    env.storage()
        .persistent()
        .set(&DataKey::Creator(token_id), metadata);
}

/// Save the creator address with an optional display name.
///
/// `verified` is initialized to `false`.
pub fn set_creator_with_name(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
    display_name: Option<String>,
) {
    let metadata = CreatorMetadata::with_details(creator.clone(), display_name, false);
    set_creator_metadata(env, token_id, &metadata);
}

/// Save the creator address with no display name.
pub fn set_creator(env: &Env, token_id: TokenId, creator: &Address) {
    set_creator_with_name(env, token_id, creator, None);
}

/// Read the full creator metadata for a token.
///
/// Returns `Err(Error::TokenNotFound)` if no creator has been recorded.
pub fn get_creator_metadata(env: &Env, token_id: TokenId) -> Result<CreatorMetadata, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Creator(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Read just the creator wallet address for a token.
///
/// Returns `Err(Error::TokenNotFound)` if no creator has been recorded.
pub fn get_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    get_creator_metadata(env, token_id).map(|m| m.creator_address)
}

/// Read the creator's display name for a token.
///
/// Returns `Ok(None)` if no display name was set, `Err(Error::TokenNotFound)`
/// if the token has no creator record at all.
pub fn get_creator_display_name(env: &Env, token_id: TokenId) -> Result<Option<String>, Error> {
    get_creator_metadata(env, token_id).map(|m| m.display_name)
}

/// Update the display name for an existing creator record.
///
/// # Errors
/// - `Error::TokenNotFound` — no creator metadata exists for this token.
pub fn set_creator_display_name(
    env: &Env,
    token_id: TokenId,
    display_name: Option<String>,
) -> Result<(), Error> {
    let mut metadata = get_creator_metadata(env, token_id)?;
    metadata.display_name = display_name;
    set_creator_metadata(env, token_id, &metadata);
    Ok(())
}

/// Check whether the creator of a token has been verified by the platform.
///
/// Returns `Err(Error::TokenNotFound)` if no creator has been recorded.
pub fn is_creator_verified(env: &Env, token_id: TokenId) -> Result<bool, Error> {
    get_creator_metadata(env, token_id).map(|m| m.verified)
}

/// Set the platform-verification flag for a token's creator.
///
/// # Errors
/// - `Error::TokenNotFound` — no creator metadata exists for this token.
pub fn set_creator_verified(env: &Env, token_id: TokenId, verified: bool) -> Result<(), Error> {
    let mut metadata = get_creator_metadata(env, token_id)?;
    metadata.verified = verified;
    set_creator_metadata(env, token_id, &metadata);
    Ok(())
}

/// Check whether creator metadata exists for a token.
pub fn creator_metadata_exists(env: &Env, token_id: TokenId) -> bool {
    env.storage().persistent().has(&DataKey::Creator(token_id))
}

/// Remove creator metadata for a token (used during rollback / cleanup).
pub fn remove_creator_metadata(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Creator(token_id));
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

// ─── Unit tests ────────────────────────────────────────────────────────────────

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

    // ── set_creator / get_creator ─────────────────────────────────────────────

    #[test]
    fn set_and_get_creator_address() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator(env, 1, &creator);
            assert_eq!(get_creator(env, 1).unwrap(), creator);
        });
    }

    #[test]
    fn get_creator_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_creator(env, 999), Err(Error::TokenNotFound));
        });
    }

    // ── set_creator_with_name ─────────────────────────────────────────────────

    #[test]
    fn set_creator_with_display_name() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let name = Some(String::from_str(env, "ClipMaster9000"));
            set_creator_with_name(env, 1, &creator, name.clone());

            let metadata = get_creator_metadata(env, 1).unwrap();
            assert_eq!(metadata.creator_address, creator);
            assert_eq!(metadata.display_name, name);
            assert!(!metadata.verified);
        });
    }

    #[test]
    fn set_creator_with_none_display_name() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator_with_name(env, 1, &creator, None);

            let metadata = get_creator_metadata(env, 1).unwrap();
            assert_eq!(metadata.creator_address, creator);
            assert_eq!(metadata.display_name, None);
            assert!(!metadata.verified);
        });
    }

    // ── set_creator_metadata / get_creator_metadata ───────────────────────────

    #[test]
    fn set_and_get_full_creator_metadata() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let expected = CreatorMetadata::with_details(
                creator.clone(),
                Some(String::from_str(env, "Alice")),
                true,
            );

            set_creator_metadata(env, 42, &expected);
            let actual = get_creator_metadata(env, 42).unwrap();

            assert_eq!(actual, expected);
            assert_eq!(actual.creator_address, creator);
            assert_eq!(actual.display_name, Some(String::from_str(env, "Alice")));
            assert!(actual.verified);
        });
    }

    #[test]
    fn get_creator_metadata_missing_returns_token_not_found() {
        with_contract(|env| {
            assert_eq!(get_creator_metadata(env, 123), Err(Error::TokenNotFound));
        });
    }

    // ── set_creator_display_name / get_creator_display_name ──────────────────

    #[test]
    fn update_display_name_after_creation() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator(env, 1, &creator);

            let new_name = Some(String::from_str(env, "NewName"));
            set_creator_display_name(env, 1, new_name.clone()).unwrap();

            assert_eq!(get_creator_display_name(env, 1).unwrap(), new_name);
            assert_eq!(get_creator(env, 1).unwrap(), creator);
            assert!(!is_creator_verified(env, 1).unwrap());
        });
    }

    #[test]
    fn clear_display_name_with_none() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator_with_name(env, 1, &creator, Some(String::from_str(env, "Temp")));

            set_creator_display_name(env, 1, None).unwrap();

            assert_eq!(get_creator_display_name(env, 1).unwrap(), None);
        });
    }

    #[test]
    fn set_display_name_missing_token_errors() {
        with_contract(|env| {
            let result = set_creator_display_name(env, 5, Some(String::from_str(env, "Nope")));
            assert_eq!(result, Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn get_display_name_missing_token_errors() {
        with_contract(|env| {
            assert_eq!(get_creator_display_name(env, 5), Err(Error::TokenNotFound));
        });
    }

    // ── set_creator_verified / is_creator_verified ────────────────────────────

    #[test]
    fn verify_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator(env, 1, &creator);

            assert!(!is_creator_verified(env, 1).unwrap());

            set_creator_verified(env, 1, true).unwrap();
            assert!(is_creator_verified(env, 1).unwrap());

            set_creator_verified(env, 1, false).unwrap();
            assert!(!is_creator_verified(env, 1).unwrap());
        });
    }

    #[test]
    fn set_verified_missing_token_errors() {
        with_contract(|env| {
            assert_eq!(
                set_creator_verified(env, 7, true),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn is_verified_missing_token_errors() {
        with_contract(|env| {
            assert_eq!(is_creator_verified(env, 7), Err(Error::TokenNotFound));
        });
    }

    // ── creator_metadata_exists ───────────────────────────────────────────────

    #[test]
    fn creator_metadata_exists_true_when_set() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator(env, 1, &creator);
            assert!(creator_metadata_exists(env, 1));
        });
    }

    #[test]
    fn creator_metadata_exists_false_when_unset() {
        with_contract(|env| {
            assert!(!creator_metadata_exists(env, 99));
        });
    }

    // ── remove_creator_metadata ───────────────────────────────────────────────

    #[test]
    fn remove_creator_metadata_clears_record() {
        with_contract(|env| {
            let creator = Address::generate(env);
            set_creator(env, 1, &creator);
            assert!(creator_metadata_exists(env, 1));

            remove_creator_metadata(env, 1);
            assert!(!creator_metadata_exists(env, 1));
            assert_eq!(get_creator(env, 1), Err(Error::TokenNotFound));
        });
    }

    // ── Per-token isolation ───────────────────────────────────────────────────

    #[test]
    fn creator_metadata_isolated_per_token() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);

            set_creator_with_name(env, 1, &alice, Some(String::from_str(env, "Alice")));
            set_creator_metadata(
                env,
                2,
                &CreatorMetadata::with_details(
                    bob.clone(),
                    Some(String::from_str(env, "Bob")),
                    true,
                ),
            );

            let m1 = get_creator_metadata(env, 1).unwrap();
            assert_eq!(m1.creator_address, alice);
            assert_eq!(m1.display_name, Some(String::from_str(env, "Alice")));
            assert!(!m1.verified);

            let m2 = get_creator_metadata(env, 2).unwrap();
            assert_eq!(m2.creator_address, bob);
            assert_eq!(m2.display_name, Some(String::from_str(env, "Bob")));
            assert!(m2.verified);
        });
    }
}
