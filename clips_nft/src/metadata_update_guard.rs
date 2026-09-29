//! Metadata update guard (Issue #1023).
//!
//! Validates all pre-conditions before any NFT metadata mutation is executed.
//!
//! # Guard sequence
//!
//! | # | Guard | Error |
//! |---|-------|-------|
//! | 1 | Token must exist | [`Error::TokenNotFound`] |
//! | 2 | Caller must be the token owner, the token creator, or a registered administrator | [`Error::Unauthorized`] |
//! | 3 | Token must not be frozen (soulbound tokens have immutable metadata) | [`Error::Unauthorized`] |
//! | 4 | Metadata update policy must permit a further update | [`Error::MetadataAlreadyUpdated`] |
//!
//! # Usage
//!
//! Call [`check_metadata_update`] at the top of every entry point that mutates
//! NFT metadata. It runs every guard in order and returns the first error.
//!
//! ```rust,ignore
//! metadata_update_guard::check_metadata_update(&env, &caller, token_id)?;
//! ```
//!
//! Individual guards are also public so callers can compose them selectively.

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::creator_storage;
use crate::frozen_token;
use crate::metadata_update_policy;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Primary entry point ──────────────────────────────────────────────────────

/// Run all metadata-update pre-condition checks.
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `caller`   — The address invoking the metadata update (must auth).
/// * `token_id` — On-chain identifier of the token whose metadata will change.
///
/// # Errors
/// | Error | Triggered when |
/// |-------|---------------|
/// | `TokenNotFound`        | Token does not exist. |
/// | `Unauthorized`         | Caller is not the owner, creator, or admin; or token is frozen. |
/// | `MetadataAlreadyUpdated` | Non-admin caller has already consumed their one-time update slot. |
pub fn check_metadata_update(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    // 1. Verify token existence.
    check_token_exists(env, token_id)?;

    // 2. Verify caller is authorized.
    check_caller_authorized(env, caller, token_id)?;

    // 3. Verify token is not frozen.
    check_not_frozen(env, token_id)?;

    // 4. Verify update policy allows the change (admins bypass this check).
    check_update_policy(env, caller, token_id)?;

    Ok(())
}

// ─── Individual guards ────────────────────────────────────────────────────────

/// Guard 1 — verify the token exists in storage.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no ownership record exists for `token_id`.
pub fn check_token_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !token_owner_storage::has_owner(env, token_id) {
        return Err(Error::TokenNotFound);
    }
    Ok(())
}

/// Guard 2 — verify the caller is permitted to update metadata for `token_id`.
///
/// A caller is authorized if they are **any** of the following:
/// 1. The token's current owner.
/// 2. The token's original creator (if a creator record exists).
/// 3. A registered contract administrator.
///
/// `require_auth` is called on the caller before the identity check so the
/// Soroban host validates the authorisation envelope.
///
/// # Errors
/// - [`Error::Unauthorized`] — `caller` does not satisfy any of the above.
pub fn check_caller_authorized(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // Always demand an authorization envelope from the caller.
    caller.require_auth();

    // 1. Token owner.
    if let Ok(owner) = token_owner_storage::get_owner(env, token_id) {
        if *caller == owner {
            return Ok(());
        }
    }

    // 2. Original creator.
    if let Ok(creator) = creator_storage::get_creator(env, token_id) {
        if *caller == creator {
            return Ok(());
        }
    }

    // 3. Registered administrator.
    if administrator_storage::is_admin(env, caller) {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

/// Guard 3 — reject metadata updates for frozen (soulbound) tokens.
///
/// A frozen token's metadata is considered immutable alongside its ownership.
///
/// # Errors
/// - [`Error::Unauthorized`] — the token is currently frozen.
pub fn check_not_frozen(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Guard 4 — enforce the one-time metadata update policy for non-admin callers.
///
/// - Registered administrators may always update metadata (unlimited updates).
/// - All other callers are limited to one update per token.
///
/// Does **not** consume the update slot; call
/// [`metadata_update_policy::mark_update_used`] after the metadata has been
/// successfully written.
///
/// # Errors
/// - [`Error::MetadataAlreadyUpdated`] — non-admin caller already used their
///   one-time update slot for `token_id`.
pub fn check_update_policy(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    // Admins bypass the one-time update restriction.
    if administrator_storage::is_admin(env, caller) {
        return Ok(());
    }

    if metadata_update_policy::is_update_used(env, token_id) {
        return Err(Error::MetadataAlreadyUpdated);
    }

    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::creator_storage;
    use crate::frozen_token;
    use crate::metadata_update_policy;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Helper: run each test inside the registered contract context ──────────

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    /// Mint a minimal token owned by `owner`.
    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    // ── Guard 1: check_token_exists ───────────────────────────────────────────

    #[test]
    fn token_exists_passes_for_minted_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_token_exists(env, 1).is_ok());
        });
    }

    #[test]
    fn token_exists_fails_for_unminted_token() {
        with_contract(|env| {
            assert_eq!(check_token_exists(env, 999), Err(Error::TokenNotFound));
        });
    }

    // ── Guard 2: check_caller_authorized ─────────────────────────────────────

    #[test]
    fn owner_is_authorized_to_update_metadata() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_caller_authorized(env, &owner, 1).is_ok());
        });
    }

    #[test]
    fn creator_is_authorized_to_update_metadata() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let creator = Address::generate(env);
            setup_token(env, 1, &owner);
            creator_storage::set_creator(env, 1, &creator);
            assert!(check_caller_authorized(env, &creator, 1).is_ok());
        });
    }

    #[test]
    fn admin_is_authorized_to_update_metadata() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            setup_token(env, 1, &owner);
            administrator_storage::add_admin(env, &admin);
            assert!(check_caller_authorized(env, &admin, 1).is_ok());
        });
    }

    #[test]
    fn stranger_is_unauthorized_to_update_metadata() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                check_caller_authorized(env, &stranger, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn revoked_admin_is_unauthorized() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            setup_token(env, 1, &owner);
            administrator_storage::add_admin(env, &admin);
            administrator_storage::remove_admin(env, &admin);
            assert_eq!(
                check_caller_authorized(env, &admin, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── Guard 3: check_not_frozen ─────────────────────────────────────────────

    #[test]
    fn unfrozen_token_passes_frozen_check() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_not_frozen(env, 1).is_ok());
        });
    }

    #[test]
    fn frozen_token_is_blocked() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            assert_eq!(check_not_frozen(env, 1), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn unfreezing_restores_metadata_update_permission() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            frozen_token::unfreeze_token(env, 1);
            assert!(check_not_frozen(env, 1).is_ok());
        });
    }

    // ── Guard 4: check_update_policy ─────────────────────────────────────────

    #[test]
    fn first_non_admin_update_is_allowed() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_update_policy(env, &owner, 1).is_ok());
        });
    }

    #[test]
    fn second_non_admin_update_is_blocked() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            metadata_update_policy::mark_update_used(env, 1);
            assert_eq!(
                check_update_policy(env, &owner, 1),
                Err(Error::MetadataAlreadyUpdated)
            );
        });
    }

    #[test]
    fn admin_can_update_after_slot_consumed() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            setup_token(env, 1, &owner);
            administrator_storage::add_admin(env, &admin);
            metadata_update_policy::mark_update_used(env, 1);
            // Admin bypasses the one-time restriction.
            assert!(check_update_policy(env, &admin, 1).is_ok());
        });
    }

    // ── Unauthorized metadata change prevention ───────────────────────────────

    #[test]
    fn stranger_cannot_change_metadata_even_if_slot_available() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            // Slot has not been consumed, yet stranger is still rejected at
            // the authorization guard before reaching the policy guard.
            assert_eq!(
                check_caller_authorized(env, &stranger, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── Full pipeline: check_metadata_update ─────────────────────────────────

    #[test]
    fn full_check_passes_for_valid_owner_update() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_metadata_update(env, &owner, 1).is_ok());
        });
    }

    #[test]
    fn full_check_fails_for_nonexistent_token() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                check_metadata_update(env, &caller, 999),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn full_check_fails_for_unauthorized_caller() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                check_metadata_update(env, &stranger, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn full_check_fails_for_frozen_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            assert_eq!(
                check_metadata_update(env, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn full_check_fails_when_update_slot_consumed() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            metadata_update_policy::mark_update_used(env, 1);
            assert_eq!(
                check_metadata_update(env, &owner, 1),
                Err(Error::MetadataAlreadyUpdated)
            );
        });
    }

    #[test]
    fn full_check_admin_bypasses_update_slot() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            setup_token(env, 1, &owner);
            administrator_storage::add_admin(env, &admin);
            metadata_update_policy::mark_update_used(env, 1);
            // Admin can still update even after the slot is consumed.
            assert!(check_metadata_update(env, &admin, 1).is_ok());
        });
    }

    #[test]
    fn full_check_creator_can_update_metadata() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let creator = Address::generate(env);
            setup_token(env, 1, &owner);
            creator_storage::set_creator(env, 1, &creator);
            assert!(check_metadata_update(env, &creator, 1).is_ok());
        });
    }

    #[test]
    fn full_check_creator_blocked_after_slot_consumed() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let creator = Address::generate(env);
            setup_token(env, 1, &owner);
            creator_storage::set_creator(env, 1, &creator);
            metadata_update_policy::mark_update_used(env, 1);
            assert_eq!(
                check_metadata_update(env, &creator, 1),
                Err(Error::MetadataAlreadyUpdated)
            );
        });
    }

    #[test]
    fn guards_are_checked_in_order_existence_before_authorization() {
        with_contract(|env| {
            // Token does not exist — we expect TokenNotFound, not Unauthorized.
            let stranger = Address::generate(env);
            assert_eq!(
                check_metadata_update(env, &stranger, 42),
                Err(Error::TokenNotFound)
            );
        });
    }
}
