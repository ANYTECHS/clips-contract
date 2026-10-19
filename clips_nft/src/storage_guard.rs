//! Storage access guard — resolves issue #1189.
//!
//! Reusable authorization checks that **must pass before any storage
//! mutation is allowed**. Each guard function:
//!
//! - Verifies the caller's permission for the requested operation.
//! - Returns a typed [`Error`] so callers can propagate failures cleanly
//!   with `?`.
//! - Calls `require_auth()` on the verified address to enforce on-chain
//!   signature checks.
//!
//! # Acceptance criteria (issue #1189)
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Verify permissions | [`guard_admin`], [`guard_token_owner`], [`guard_operator`] |
//! | Return storage errors | [`guard_token_exists`] returns [`Error::StorageNotFound`] |
//! | Reusable before storage mutations | All guards are standalone `fn`s; compose them freely |
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::storage_guard::{guard_admin, guard_not_paused, guard_token_owner};
//!
//! pub fn update_metadata(env: Env, caller: Address, token_id: TokenId) -> Result<(), Error> {
//!     guard_not_paused(&env)?;
//!     guard_token_owner(&env, &caller, token_id)?;
//!     // ... safe to mutate storage here
//!     Ok(())
//! }
//! ```

use soroban_sdk::{Address, Env};

use crate::storage::keys::StorageKey;
use crate::types::{DataKey, Error, TokenData, TokenId};

// ─── Admin guards ─────────────────────────────────────────────────────────────

/// Verify the caller is the stored contract admin and call `require_auth`.
///
/// Must be called before any admin-only storage mutation.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract has not been initialised yet.
/// - [`Error::Unauthorized`] — `caller` does not match the stored admin address.
pub fn guard_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    if *caller != admin {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

/// Verify the caller is the stored contract admin **or** the pending owner
/// nominated in a two-step ownership transfer.
///
/// # Errors
/// - [`Error::NotInitialized`] — contract not yet initialised.
/// - [`Error::Unauthorized`] — caller is neither admin nor pending owner.
pub fn guard_admin_or_pending(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;

    if *caller == admin {
        caller.require_auth();
        return Ok(());
    }

    let pending: Option<Address> = env.storage().instance().get(&DataKey::PendingOwner);
    if let Some(pending_owner) = pending {
        if *caller == pending_owner {
            caller.require_auth();
            return Ok(());
        }
    }

    Err(Error::Unauthorized)
}

// ─── Pause guards ─────────────────────────────────────────────────────────────

/// Verify the contract is **not** currently paused.
///
/// Must be called before any user-facing mutation (mint, transfer, approve, …).
///
/// # Errors
/// - [`Error::ContractPaused`] — the contract is paused and mutations are
///   blocked until an admin calls `unpause`.
pub fn guard_not_paused(env: &Env) -> Result<(), Error> {
    if env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
    {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

/// Verify the contract **is** currently paused (for operations that only make
/// sense while paused, such as `unpause` or emergency recovery).
///
/// # Errors
/// - [`Error::NotPaused`] — the contract is not paused.
pub fn guard_is_paused(env: &Env) -> Result<(), Error> {
    if !env
        .storage()
        .instance()
        .get::<_, bool>(&DataKey::Paused)
        .unwrap_or(false)
    {
        return Err(Error::NotPaused);
    }
    Ok(())
}

// ─── Token existence guards ───────────────────────────────────────────────────

/// Verify that `token_id` exists in persistent storage.
///
/// Returns [`Error::StorageNotFound`] (a storage-layer error) so callers can
/// distinguish a missing token from a permission failure.
///
/// # Errors
/// - [`Error::StorageNotFound`] — token with `token_id` does not exist.
pub fn guard_token_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Token(token_id)) {
        return Err(Error::StorageNotFound);
    }
    Ok(())
}

// ─── Ownership guards ─────────────────────────────────────────────────────────

/// Verify the caller is the owner of `token_id` and call `require_auth`.
///
/// # Errors
/// - [`Error::StorageNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — caller is not the token owner.
pub fn guard_token_owner(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;
    if data.owner != *caller {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

/// Load the token and verify caller ownership, returning the [`TokenData`] on
/// success so callers avoid a second storage read.
///
/// # Errors
/// - [`Error::StorageNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — caller is not the token owner.
pub fn guard_token_owner_load(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<TokenData, Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;
    if data.owner != *caller {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(data)
}

// ─── Operator / approval guards ───────────────────────────────────────────────

/// Verify the caller is either the owner or an approved operator for `token_id`.
///
/// An "approved operator" is either:
/// - The address approved for this specific token via `approve`, or
/// - An address approved for all of the owner's tokens via `set_approval_for_all`.
///
/// # Errors
/// - [`Error::StorageNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — caller is neither owner nor an approved operator.
pub fn guard_operator(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;

    // Owner always has permission.
    if data.owner == *caller {
        caller.require_auth();
        return Ok(());
    }

    // Check single-token approval (using the modular StorageKey).
    let approved: Option<Address> = env
        .storage()
        .persistent()
        .get(&StorageKey::Approval(token_id));
    if let Some(approved_addr) = approved {
        if approved_addr == *caller {
            caller.require_auth();
            return Ok(());
        }
    }

    // Check global operator approval: (owner, operator) → bool.
    let approved_for_all: bool = env
        .storage()
        .persistent()
        .get::<_, bool>(&StorageKey::OperatorApproval(
            data.owner.clone(),
            caller.clone(),
        ))
        .unwrap_or(false);
    if approved_for_all {
        caller.require_auth();
        return Ok(());
    }

    Err(Error::Unauthorized)
}

/// Verify the caller is the **direct owner** of the token — not a delegated
/// operator.
///
/// Use this guard when an action is strictly owner-only (e.g. burning) and
/// must not be delegated.
///
/// # Errors
/// - [`Error::StorageNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — caller is not the direct owner.
pub fn guard_strict_owner(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let data: TokenData = env
        .storage()
        .persistent()
        .get(&DataKey::Token(token_id))
        .ok_or(Error::StorageNotFound)?;
    if data.owner != *caller {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

// ─── Composite helpers ────────────────────────────────────────────────────────

/// Run multiple pre-computed guard results in sequence, short-circuiting on
/// the first failure.
///
/// # Example
///
/// ```rust,ignore
/// guard_all([
///     guard_not_paused(&env),
///     guard_token_owner(&env, &caller, token_id),
/// ])?;
/// ```
pub fn guard_all<const N: usize>(guards: [Result<(), Error>; N]) -> Result<(), Error> {
    for result in guards {
        result?;
    }
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataKey, TokenData};
    use soroban_sdk::{testutils::Address as _, Env};

    // ── guard_admin ──────────────────────────────────────────────────────────

    #[test]
    fn test_guard_admin_not_initialized() {
        let env = Env::default();
        let addr = Address::generate(&env);
        assert_eq!(guard_admin(&env, &addr), Err(Error::NotInitialized));
    }

    #[test]
    fn test_guard_admin_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        assert_eq!(guard_admin(&env, &other), Err(Error::Unauthorized));
    }

    #[test]
    fn test_guard_admin_success() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        assert!(guard_admin(&env, &admin).is_ok());
    }

    // ── guard_not_paused ─────────────────────────────────────────────────────

    #[test]
    fn test_guard_not_paused_when_paused() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::Paused, &true);
        assert_eq!(guard_not_paused(&env), Err(Error::ContractPaused));
    }

    #[test]
    fn test_guard_not_paused_when_unpaused() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::Paused, &false);
        assert!(guard_not_paused(&env).is_ok());
    }

    #[test]
    fn test_guard_not_paused_default_not_paused() {
        // Key absent → defaults to false → should pass.
        let env = Env::default();
        assert!(guard_not_paused(&env).is_ok());
    }

    // ── guard_is_paused ──────────────────────────────────────────────────────

    #[test]
    fn test_guard_is_paused_when_not_paused() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::Paused, &false);
        assert_eq!(guard_is_paused(&env), Err(Error::NotPaused));
    }

    #[test]
    fn test_guard_is_paused_success() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::Paused, &true);
        assert!(guard_is_paused(&env).is_ok());
    }

    // ── guard_token_exists ───────────────────────────────────────────────────

    #[test]
    fn test_guard_token_exists_missing() {
        let env = Env::default();
        assert_eq!(guard_token_exists(&env, 42), Err(Error::StorageNotFound));
    }

    #[test]
    fn test_guard_token_exists_present() {
        let env = Env::default();
        let owner = Address::generate(&env);
        env.storage().persistent().set(
            &DataKey::Token(1),
            &TokenData {
                owner: owner.clone(),
                clip_id: 10,
            },
        );
        assert!(guard_token_exists(&env, 1).is_ok());
    }

    // ── guard_token_owner ────────────────────────────────────────────────────

    #[test]
    fn test_guard_token_owner_token_missing() {
        let env = Env::default();
        let addr = Address::generate(&env);
        assert_eq!(
            guard_token_owner(&env, &addr, 99),
            Err(Error::StorageNotFound)
        );
    }

    #[test]
    fn test_guard_token_owner_not_owner() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage().persistent().set(
            &DataKey::Token(0),
            &TokenData {
                owner: owner.clone(),
                clip_id: 1,
            },
        );
        assert_eq!(
            guard_token_owner(&env, &other, 0),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn test_guard_token_owner_success() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        env.storage().persistent().set(
            &DataKey::Token(0),
            &TokenData {
                owner: owner.clone(),
                clip_id: 1,
            },
        );
        assert!(guard_token_owner(&env, &owner, 0).is_ok());
    }

    // ── guard_strict_owner ───────────────────────────────────────────────────

    #[test]
    fn test_guard_strict_owner_success() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        env.storage().persistent().set(
            &DataKey::Token(5),
            &TokenData {
                owner: owner.clone(),
                clip_id: 5,
            },
        );
        assert!(guard_strict_owner(&env, &owner, 5).is_ok());
    }

    #[test]
    fn test_guard_strict_owner_unauthorized() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage().persistent().set(
            &DataKey::Token(5),
            &TokenData {
                owner: owner.clone(),
                clip_id: 5,
            },
        );
        assert_eq!(
            guard_strict_owner(&env, &other, 5),
            Err(Error::Unauthorized)
        );
    }

    // ── guard_all ────────────────────────────────────────────────────────────

    #[test]
    fn test_guard_all_passes() {
        assert!(guard_all::<2>([Ok(()), Ok(())]).is_ok());
    }

    #[test]
    fn test_guard_all_first_fails() {
        assert_eq!(
            guard_all::<2>([Err(Error::Unauthorized), Ok(())]),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn test_guard_all_second_fails() {
        assert_eq!(
            guard_all::<2>([Ok(()), Err(Error::ContractPaused)]),
            Err(Error::ContractPaused)
        );
    }
}
