//! Freeze state guard (Issue #1094).
//!
//! Implements a guard that prevents operations against frozen NFTs.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Retrieve NFT freeze state | [`get_freeze_state`] |
//! | Reject operations on frozen tokens | [`require_not_frozen`] |
//! | Allow permitted admin operations | [`require_not_frozen_or_admin`] |
//! | Add freeze-state tests | Comprehensive test module |
//!
//! # Freeze semantics
//!
//! A frozen (soulbound) token is permanently non-transferable and has immutable
//! metadata. Freezing is irreversible and should only be used for special cases
//! (e.g., commemorative NFTs, author originals).
//!
//! - **Transfer restrictions** — frozen tokens cannot be transferred.
//! - **Metadata immutability** — frozen tokens cannot have metadata updated.
//! - **Admin override** — certain administrative operations may bypass freeze
//!   restrictions (e.g., emergency burn, reassign creator).
//!
//! # Usage
//!
//! ```rust,ignore
//! // Standard guard — reject frozen tokens
//! freeze_state_guard::require_not_frozen(&env, token_id)?;
//!
//! // Admin variant — allow frozen tokens for admin operations
//! freeze_state_guard::require_not_frozen_or_admin(&env, &admin, token_id)?;
//!
//! // Check freeze state without error
//! if freeze_state_guard::get_freeze_state(&env, token_id) {
//!     // token is frozen; operations restricted
//! }
//! ```

use soroban_sdk::{Address, Env};

use crate::frozen_token;
use crate::admin_access_control_guard;
use crate::types::TokenId;

// ─── Primary integration point ────────────────────────────────────────────────

/// Require that `token_id` is not frozen.
///
/// Returns `Ok(())` if the token is not frozen, or [`crate::types::Error::Unauthorized`]
/// if it is frozen. This is the standard guard for operations that must reject
/// frozen tokens (e.g., transfers, metadata updates).
///
/// # Errors
/// - [`crate::types::Error::Unauthorized`] — token is currently frozen.
///
/// # Example
///
/// ```rust,ignore
/// freeze_state_guard::require_not_frozen(&env, token_id)?;
/// // If Ok, token is not frozen; safe to proceed
/// ```
pub fn require_not_frozen(env: &Env, token_id: TokenId) -> Result<(), crate::types::Error> {
    if get_freeze_state(env, token_id) {
        return Err(crate::types::Error::Unauthorized);
    }
    Ok(())
}

/// Require that `token_id` is not frozen, **unless** `caller` is an admin.
///
/// Administrative operations (e.g. emergency burn, creator reassignment) may
/// bypass freeze restrictions. This variant allows admins to operate on frozen
/// tokens while still protecting non-admin callers from accidental modifications.
///
/// Returns `Ok(())` if **any** of the following holds:
///
/// 1. The token is not frozen.
/// 2. The caller is an authorized administrator.
///
/// # Errors
/// - [`crate::types::Error::Unauthorized`] — token is frozen and caller is not an admin.
///
/// # Example
///
/// ```rust,ignore
/// freeze_state_guard::require_not_frozen_or_admin(&env, &caller, token_id)?;
/// // If Ok, either token is not frozen or caller is an authorized admin
/// ```
pub fn require_not_frozen_or_admin(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), crate::types::Error> {
    if !get_freeze_state(env, token_id) {
        return Ok(());
    }

    // Token is frozen; check if caller is an admin
    if admin_access_control_guard::check_caller_is_admin(env, caller) {
        return Ok(());
    }

    Err(crate::types::Error::Unauthorized)
}

// ─── Individual freeze state probes ────────────────────────────────────────────

/// Return `true` when `token_id` is currently frozen (soulbound).
///
/// Performs no error handling—purely a read-only check. Use this when you need
/// a boolean result or are building custom logic around frozen state.
///
/// A frozen token is permanently non-transferable and has immutable metadata.
///
/// # Example
///
/// ```rust,ignore
/// if freeze_state_guard::get_freeze_state(&env, token_id) {
///     // token is frozen; restrictions apply
/// }
/// ```
pub fn get_freeze_state(env: &Env, token_id: TokenId) -> bool {
    frozen_token::is_frozen(env, token_id)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frozen_token;
    use crate::administrator_storage;
    use crate::token_owner_storage;
    use crate::types::DataKey;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ───────────────────────────────────────────────────────

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

    fn setup_contract_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    // ── require_not_frozen ─────────────────────────────────────────────────

    #[test]
    fn require_not_frozen_passes_for_unfrozen_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(require_not_frozen(env, token_id).is_ok());
        });
    }

    #[test]
    fn require_not_frozen_fails_for_frozen_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);

            assert_eq!(
                require_not_frozen(env, token_id),
                Err(crate::types::Error::Unauthorized)
            );
        });
    }

    // ── require_not_frozen_or_admin ────────────────────────────────────────

    #[test]
    fn require_not_frozen_or_admin_passes_for_unfrozen_token_non_admin() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(require_not_frozen_or_admin(env, &caller, token_id).is_ok());
        });
    }

    #[test]
    fn require_not_frozen_or_admin_passes_for_frozen_token_admin() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);
            setup_contract_admin(env, &admin);

            assert!(require_not_frozen_or_admin(env, &admin, token_id).is_ok());
        });
    }

    #[test]
    fn require_not_frozen_or_admin_passes_for_frozen_token_multi_admin() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            let multi_admin = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);
            setup_contract_admin(env, &admin);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(require_not_frozen_or_admin(env, &multi_admin, token_id).is_ok());
        });
    }

    #[test]
    fn require_not_frozen_or_admin_fails_for_frozen_token_non_admin() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let caller = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);

            assert_eq!(
                require_not_frozen_or_admin(env, &caller, token_id),
                Err(crate::types::Error::Unauthorized)
            );
        });
    }

    // ── get_freeze_state ───────────────────────────────────────────────────

    #[test]
    fn get_freeze_state_returns_false_for_unfrozen_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            assert!(!get_freeze_state(env, token_id));
        });
    }

    #[test]
    fn get_freeze_state_returns_true_for_frozen_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);

            assert!(get_freeze_state(env, token_id));
        });
    }

    // ── Multiple tokens ────────────────────────────────────────────────────

    #[test]
    fn freeze_state_is_independent_across_tokens() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            setup_token(env, 2, &owner);
            setup_token(env, 3, &owner);

            // Freeze only token 2
            frozen_token::freeze_token(env, 2);

            assert!(!get_freeze_state(env, 1));
            assert!(get_freeze_state(env, 2));
            assert!(!get_freeze_state(env, 3));
        });
    }

    // ── Irreversibility ────────────────────────────────────────────────────

    #[test]
    fn frozen_token_remains_frozen_after_unfreeze_is_not_called() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);

            frozen_token::freeze_token(env, token_id);
            assert!(get_freeze_state(env, token_id));

            // Check multiple times; should remain frozen
            assert!(get_freeze_state(env, token_id));
            assert!(get_freeze_state(env, token_id));
        });
    }

    // ── Admin privileges ──────────────────────────────────────────────────

    #[test]
    fn admin_override_allows_operations_on_frozen_tokens() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            let non_admin = Address::generate(env);
            let token_id = 1;

            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);
            setup_contract_admin(env, &admin);

            // Admin can proceed
            assert!(require_not_frozen_or_admin(env, &admin, token_id).is_ok());
            // Non-admin cannot
            assert_eq!(
                require_not_frozen_or_admin(env, &non_admin, token_id),
                Err(crate::types::Error::Unauthorized)
            );
        });
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn get_freeze_state_is_deterministic_across_calls() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let token_id = 1;
            setup_token(env, token_id, &owner);
            frozen_token::freeze_token(env, token_id);

            // Multiple calls should return consistent results
            assert!(get_freeze_state(env, token_id));
            assert!(get_freeze_state(env, token_id));
            assert!(get_freeze_state(env, token_id));
        });
    }

    #[test]
    fn freeze_state_for_nonexistent_token_returns_false() {
        with_contract(|env| {
            let nonexistent = 999;
            assert!(!get_freeze_state(env, nonexistent));
        });
    }
}
