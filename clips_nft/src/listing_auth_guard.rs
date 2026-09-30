//! Listing Authorization Guard (Issue #1070).
//!
//! Verifies whether a caller is authorized to create or modify a marketplace
//! listing for a specific NFT.  This guard is the canonical authorization check
//! for all listing entry points in the contract.
//!
//! # Acceptance criteria
//!
//! | Criterion | Guard function |
//! |-----------|---------------|
//! | Verify NFT ownership or approved authority | [`check_listing_authority`] |
//! | Reject unauthorized listing creation | enforced by [`require_listing_auth`] |
//! | Validate NFT state (frozen token check) | [`check_token_is_listable`] |
//! | Add listing authorization tests | see `#[cfg(test)]` block below |
//!
//! # Authorization hierarchy
//!
//! A caller may create or modify a listing only when **all** of the following
//! conditions hold:
//!
//! 1. **Token exists** — the referenced `token_id` has an on-chain ownership record.
//! 2. **Token is listable** — the token must not be frozen (soulbound tokens are
//!    permanently non-transferable and therefore cannot be listed).
//! 3. **Caller is authorized** — the caller must be any of:
//!    - The token's current owner.
//!    - An address with a single-token approval for `token_id`.
//!    - An operator approved for all of the owner's tokens.
//!
//! # Usage
//!
//! ```rust,ignore
//! listing_auth_guard::require_listing_auth(&env, &caller, token_id)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::frozen_token;
use crate::operator_approval;
use crate::token_approval;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Primary entry point ──────────────────────────────────────────────────────

/// Validate that `caller` is authorized to create or modify a listing for
/// `token_id`.
///
/// Performs all listing pre-condition checks in order:
/// 1. Token exists — returns [`Error::TokenNotFound`] if missing.
/// 2. Token is listable — returns [`Error::Unauthorized`] if frozen.
/// 3. Caller is authorized — returns [`Error::Unauthorized`] if none of the
///    ownership/approval checks pass.
///
/// `caller.require_auth()` is called before any storage reads so that the
/// Soroban host always validates the cryptographic authorization envelope.
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `caller`   — Address attempting to create or modify the listing.
/// * `token_id` — Token to be listed.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `TokenNotFound` | No ownership record exists for `token_id`. |
/// | `Unauthorized`  | Token is frozen, or `caller` lacks listing authority. |
pub fn require_listing_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // Demand a signed authorization envelope before any storage reads.
    caller.require_auth();

    // 1. Token must exist; retrieve its owner.
    let owner = token_owner_storage::get_owner(env, token_id)?;

    // 2. Token must not be frozen (soulbound tokens cannot be transferred).
    check_token_is_listable(env, token_id)?;

    // 3. Caller must have authority over this token.
    check_listing_authority(env, caller, &owner, token_id)?;

    Ok(())
}

// ─── Token state check ────────────────────────────────────────────────────────

/// Validate that `token_id` is in a state that permits listing.
///
/// Frozen (soulbound) tokens are permanently non-transferable.  Creating a
/// listing for such a token would mislead buyers, so this guard blocks it.
///
/// # Errors
/// Returns [`Error::Unauthorized`] if `token_id` is currently frozen.
pub fn check_token_is_listable(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

// ─── Caller authority check ───────────────────────────────────────────────────

/// Validate that `caller` has authority to list `token_id` owned by `owner`.
///
/// A caller is authorized if they are **any** of the following:
///
/// 1. The current `owner` of the token.
/// 2. An address with a single-token approval for `token_id` (ERC-721
///    `approve` analogue).
/// 3. An operator approved for all of `owner`'s tokens (`setApprovalForAll`
///    analogue).
///
/// # Errors
/// Returns [`Error::Unauthorized`] if none of the above conditions is met.
pub fn check_listing_authority(
    env: &Env,
    caller: &Address,
    owner: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // 1. Owner may always create a listing for their own token.
    if caller == owner {
        return Ok(());
    }

    // 2. Single-token approved address.
    if let Some(approved) = token_approval::get_approval(env, token_id) {
        if &approved == caller {
            return Ok(());
        }
    }

    // 3. Operator approved for all tokens of `owner`.
    if operator_approval::is_operator(env, owner, caller) {
        return Ok(());
    }

    Err(Error::Unauthorized)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frozen_token;
    use crate::operator_approval;
    use crate::token_approval;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    // ── Ownership authorization ───────────────────────────────────────────────

    #[test]
    fn owner_can_list_their_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(require_listing_auth(env, &owner, 1).is_ok());
        });
    }

    #[test]
    fn non_owner_without_approval_cannot_list() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(require_listing_auth(env, &stranger, 1), Err(Error::Unauthorized));
        });
    }

    // ── Single-token approval ─────────────────────────────────────────────────

    #[test]
    fn approved_address_can_list_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            assert!(require_listing_auth(env, &approved, 1).is_ok());
        });
    }

    #[test]
    fn revoked_approval_cannot_list_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            token_approval::remove_approval(env, 1);
            assert_eq!(
                require_listing_auth(env, &approved, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn approval_for_different_token_does_not_grant_listing_rights() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);
            token_approval::save_approval(env, 2, &approved);
            // Approved for token 2, not token 1.
            assert_eq!(
                require_listing_auth(env, &approved, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── Operator approval ─────────────────────────────────────────────────────

    #[test]
    fn operator_can_list_any_token_of_the_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);
            assert!(require_listing_auth(env, &operator, 1).is_ok());
        });
    }

    #[test]
    fn revoked_operator_cannot_list_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);
            operator_approval::remove_operator(env, &owner, &operator);
            assert_eq!(
                require_listing_auth(env, &operator, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn operator_for_different_owner_cannot_list_token() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner_a);
            // Operator approved for owner_b, not owner_a.
            operator_approval::save_operator(env, &owner_b, &operator);
            assert_eq!(
                require_listing_auth(env, &operator, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── Token state validation ────────────────────────────────────────────────

    #[test]
    fn frozen_token_cannot_be_listed_by_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            assert_eq!(require_listing_auth(env, &owner, 1), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn frozen_token_cannot_be_listed_by_approved_address() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            frozen_token::freeze_token(env, 1);
            assert_eq!(
                require_listing_auth(env, &approved, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn unfrozen_token_can_be_listed_again() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            frozen_token::unfreeze_token(env, 1);
            assert!(require_listing_auth(env, &owner, 1).is_ok());
        });
    }

    #[test]
    fn check_token_is_listable_passes_for_active_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(check_token_is_listable(env, 1).is_ok());
        });
    }

    #[test]
    fn check_token_is_listable_fails_for_frozen_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            assert_eq!(check_token_is_listable(env, 1), Err(Error::Unauthorized));
        });
    }

    // ── Token existence ───────────────────────────────────────────────────────

    #[test]
    fn listing_nonexistent_token_returns_token_not_found() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(require_listing_auth(env, &caller, 999), Err(Error::TokenNotFound));
        });
    }
}
