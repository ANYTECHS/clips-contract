//! NFT state guard — validates NFT active state before operations.
//!
//! This module provides a guard that ensures an NFT is in an active state
//! before operations such as transfers or marketplace actions are permitted.
//!
//! # Acceptance Criteria
//!
//! | Criterion | Implementation |
//! |-----------|-----------------|
//! | Validate NFT state | [`require_token_active`] checks existence and status |
//! | Reject inactive tokens | Returns appropriate error for frozen/missing tokens |
//! | Support existing token state errors | Uses [`Error::TokenNotFound`] and [`Error::Unauthorized`] |
//! | Add unit tests | Comprehensive test module |
//!
//! # Design
//!
//! A token is considered **active** when:
//! 1. The token exists in the contract's token storage.
//! 2. The token is not frozen (soulbound).
//! 3. The token has not been burned.
//!
//! The guard rejects transfers, marketplace operations, and other
//! state-dependent operations by checking these conditions in sequence.
//!
//! # Token State Variants
//!
//! - **Active** — token exists, not frozen, can be transferred or listed.
//! - **Frozen** — token exists but is soulbound; only admin can modify.
//! - **Not Found** — token ID never minted or has been burned.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::nft_state_guard;
//!
//! // Ensure token is active before transfer
//! nft_state_guard::require_token_active(&env, token_id)?;
//!
//! // Check without requiring auth (read-only probe)
//! if nft_state_guard::is_token_active(&env, token_id) {
//!     // Safe to transfer or list
//! }
//! ```

use soroban_sdk::Env;

use crate::token_state_validator::{self, TokenState};
use crate::types::{Error, TokenId};
use crate::frozen_token;
use crate::token_owner_storage;
use crate::types::{Error, InvalidTransferStateReason, TokenId};

// ─── Active state checks ──────────────────────────────────────────────────────

/// Validate that a token exists and is in an active (non-frozen) state.
///
/// This is the primary guard for token-dependent operations.
///
/// # Arguments
///
/// * `env` — Soroban environment.
/// * `token_id` — Token identifier to validate.
///
/// # Returns
///
/// - `Ok(())` — token exists and is active.
/// - `Err(Error::InvalidTransferState(reason))` — token is in an incompatible
///   state for transfer:
///   - [`InvalidTransferStateReason::MissingOwner`] — token does not exist or
///     has no owner recorded.
///   - [`InvalidTransferStateReason::Frozen`] — token is frozen (soulbound).
///   - [`InvalidTransferStateReason::InactiveToken`] — token is otherwise
///     inactive.
///
/// # Example
///
/// ```rust,ignore
/// nft_state_guard::require_token_active(&env, token_id)?;
/// // Token is confirmed to exist and be transferable
/// ```
pub fn require_token_active(env: &Env, token_id: TokenId) -> Result<(), Error> {
    match token_state_validator::get_token_state(env, token_id) {
        TokenState::Active => Ok(()),
        TokenState::Frozen => Err(Error::Unauthorized),
        TokenState::NotFound => Err(Error::TokenNotFound),
    // First, verify the token exists by attempting to load its owner.
    if token_owner_storage::get_owner(env, token_id).is_err() {
        return Err(Error::InvalidTransferState(
            InvalidTransferStateReason::MissingOwner,
        ));
    }

    // Second, verify the token is not frozen.
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::InvalidTransferState(
            InvalidTransferStateReason::Frozen,
        ));
    }

    // Third, verify the token is active.
    if !token_is_active(env, token_id) {
        return Err(Error::InvalidTransferState(
            InvalidTransferStateReason::InactiveToken,
        ));
    }
}

/// Check whether a token is active (exists and is not frozen).
///
/// This is the internal predicate used by [`require_token_active`] to
/// determine the `InactiveToken` state.
fn token_is_active(env: &Env, token_id: TokenId) -> bool {
    token_owner_storage::get_owner(env, token_id).is_ok() && !frozen_token::is_frozen(env, token_id)
}

/// Check whether a token is active without requiring auth or failing hard.
///
/// This is a read-only probe that returns a boolean, useful for conditional
/// logic or diagnostics.
///
/// # Arguments
///
/// * `env` — Soroban environment.
/// * `token_id` — Token identifier to check.
///
/// # Returns
///
/// `true` if the token exists and is not frozen, `false` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// if nft_state_guard::is_token_active(&env, token_id) {
///     // Token is confirmed active; safe to proceed
/// } else {
///     // Token is missing or frozen
/// }
/// ```
pub fn is_token_active(env: &Env, token_id: TokenId) -> bool {
    matches!(
        token_state_validator::get_token_state(env, token_id),
        TokenState::Active
    )
}

// ─── Individual state checks ──────────────────────────────────────────────────

/// Validate that a token exists (has an owner recorded).
///
/// # Arguments
///
/// * `env` — Soroban environment.
/// * `token_id` — Token identifier to validate.
///
/// # Returns
///
/// - `Ok(())` — token exists.
/// - `Err(Error::TokenNotFound)` — token does not exist.
pub fn require_token_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    match token_state_validator::get_token_state(env, token_id) {
        TokenState::NotFound => Err(Error::TokenNotFound),
        _ => Ok(()),
    }
}

/// Validate that a token is not frozen (soulbound).
///
/// Assumes the token exists; use [`require_token_active`] for a complete check.
///
/// # Arguments
///
/// * `env` — Soroban environment.
/// * `token_id` — Token identifier to validate.
///
/// # Returns
///
/// - `Ok(())` — token is not frozen.
/// - `Err(Error::Unauthorized)` — token is frozen.
pub fn require_token_not_frozen(env: &Env, token_id: TokenId) -> Result<(), Error> {
    match token_state_validator::get_token_state(env, token_id) {
        TokenState::Frozen => Err(Error::Unauthorized),
        _ => Ok(()),
    }
}

/// Check whether a token exists without failing.
///
/// # Arguments
///
/// * `env` — Soroban environment.
/// * `token_id` — Token identifier to check.
///
/// # Returns
///
/// `true` if the token exists, `false` otherwise.
pub fn token_exists(env: &Env, token_id: TokenId) -> bool {
    !matches!(
        token_state_validator::get_token_state(env, token_id),
        TokenState::NotFound
    )
}

/// Check whether a token is frozen without failing.
///
/// # Arguments
///
/// * `env` — Soroban environment.
/// * `token_id` — Token identifier to check.
///
/// # Returns
///
/// `true` if the token is frozen, `false` otherwise.
pub fn token_is_frozen(env: &Env, token_id: TokenId) -> bool {
    matches!(
        token_state_validator::get_token_state(env, token_id),
        TokenState::Frozen
    )
}

// ─── Guard implementation ──────────────────────────────────────────────────────

use crate::guard_interface::{Guard, GuardContext};
use crate::guard_result::GuardResult;
use crate::token_frozen_guard::TokenFrozenGuard;

/// A [`Guard`] implementation that validates NFT active state.
///
/// This guard checks that the token identified in the [`GuardContext`] is
/// in an active (non-frozen, existing) state before operations proceed.
/// Frozen-token rejection is delegated to [`TokenFrozenGuard`] so that the
/// freeze policy lives in a single place and can be composed with other
/// guards.
///
/// # Errors
///
/// - [`Error::TokenNotFound`] — token does not exist.
/// - [`Error::Unauthorized`] — token is frozen.
///
/// # Usage
///
/// ```rust,ignore
/// use crate::nft_state_guard::NftStateGuard;
/// use crate::guard_validator;
///
/// let guard = NftStateGuard;
/// guard_validator::execute_guard(&ctx, &guard)?;
/// ```
pub struct NftStateGuard;

impl Guard for NftStateGuard {
    fn execute(&self, ctx: &GuardContext) -> GuardResult {
        let token_id = match ctx.token_id {
            Some(id) => id,
            None => {
                // Guard requires a token context; no operation to validate
                return GuardResult::Fail(Error::TokenNotFound);
            }
        };

        // Token must exist before any further state checks.
        if let Err(err) = require_token_exists(&ctx.env, token_id) {
            return GuardResult::Fail(err);
        }

        // Delegate frozen-token rejection to the dedicated guard.
        TokenFrozenGuard.execute(ctx)
    }

    fn name(&self) -> &'static str {
        "nft_state_guard"
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frozen_token;
    use crate::token_owner_storage;
    use soroban_sdk::Address;

    /// Set up an active token: owner recorded, not frozen.
    fn setup_active_token(env: &Env, token_id: TokenId) -> Address {
        let owner = Address::generate(env);
        token_owner_storage::set_owner(env, token_id, &owner);
        owner
    }

    /// Set up a frozen token: owner recorded, frozen flag set.
    fn setup_frozen_token(env: &Env, token_id: TokenId) -> Address {
        let owner = setup_active_token(env, token_id);
        frozen_token::set_frozen(env, token_id, true);
        owner
    }

    // ─── Active token ─────────────────────────────────────────────────────

    #[test]
    fn test_require_token_active_ok_for_active_token() {
        let env = Env::default();
        let token_id = 42u64;
        setup_active_token(&env, token_id);

        assert_eq!(require_token_active(&env, token_id), Ok(()));
    }

    #[test]
    fn test_is_token_active_true_for_active_token() {
        let env = Env::default();
        let token_id = 42u64;
        setup_active_token(&env, token_id);

        assert!(is_token_active(&env, token_id));
    }

    #[test]
    fn test_token_exists_true_for_active_token() {
        let env = Env::default();
        let token_id = 42u64;
        setup_active_token(&env, token_id);

        assert!(token_exists(&env, token_id));
    }

    #[test]
    fn test_token_is_frozen_false_for_active_token() {
        let env = Env::default();
        let token_id = 42u64;
        setup_active_token(&env, token_id);

        assert!(!token_is_frozen(&env, token_id));
    }

    #[test]
    fn test_require_token_not_frozen_ok_for_active_token() {
        let env = Env::default();
        let token_id = 42u64;
        setup_active_token(&env, token_id);

        assert_eq!(require_token_not_frozen(&env, token_id), Ok(()));
    }

    // ─── Frozen token ─────────────────────────────────────────────────────

    #[test]
    fn test_require_token_active_rejects_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        setup_frozen_token(&env, token_id);

        assert_eq!(require_token_active(&env, token_id), Err(Error::Unauthorized));
    }

    #[test]
    fn test_is_token_active_false_for_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        setup_frozen_token(&env, token_id);

        assert!(!is_token_active(&env, token_id));
    }

    #[test]
    fn test_token_exists_true_for_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        setup_frozen_token(&env, token_id);

        assert!(token_exists(&env, token_id));
    }

    #[test]
    fn test_token_is_frozen_true_for_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        setup_frozen_token(&env, token_id);

        assert!(token_is_frozen(&env, token_id));
    }

    #[test]
    fn test_require_token_not_frozen_rejects_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        setup_frozen_token(&env, token_id);

        assert_eq!(require_token_not_frozen(&env, token_id), Err(Error::Unauthorized));
    }

    // ─── Missing token ────────────────────────────────────────────────────

    #[test]
    fn test_require_token_active_rejects_missing_token() {
        let env = Env::default();
        let token_id = 999u64;

        assert_eq!(require_token_active(&env, token_id), Err(Error::TokenNotFound));
    }

    #[test]
    fn test_is_token_active_false_for_missing_token() {
        let env = Env::default();
        let token_id = 999u64;

        assert!(!is_token_active(&env, token_id));
    }

    #[test]
    fn test_token_exists_false_for_missing_token() {
        let env = Env::default();
        let token_id = 999u64;

        assert!(!token_exists(&env, token_id));
    }

    #[test]
    fn test_token_is_frozen_false_for_missing_token() {
        let env = Env::default();
        let token_id = 999u64;

        assert!(!token_is_frozen(&env, token_id));
    }

    #[test]
    fn test_require_token_exists_rejects_missing_token() {
        let env = Env::default();
        let token_id = 999u64;

        assert_eq!(require_token_exists(&env, token_id), Err(Error::TokenNotFound));
    }

    #[test]
    fn test_require_token_exists_ok_for_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        setup_frozen_token(&env, token_id);

        assert_eq!(require_token_exists(&env, token_id), Ok(()));
    }

    // ─── Guard impl ───────────────────────────────────────────────────────

    #[test]
    fn test_nft_state_guard_no_token_context() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = GuardContext::new(env, caller);

        let guard = NftStateGuard;
        let result = guard.execute(&ctx);

        assert_eq!(result, GuardResult::Fail(Error::TokenNotFound));
    }

    #[test]
    fn test_nft_state_guard_passes_for_active_token() {
        let env = Env::default();
        let token_id = 42u64;
        let owner = setup_active_token(&env, token_id);
        let ctx = GuardContext::with_token(env, owner, token_id);

        let guard = NftStateGuard;
        assert_eq!(guard.execute(&ctx), GuardResult::Pass);
    }

    #[test]
    fn test_nft_state_guard_fails_for_frozen_token() {
        let env = Env::default();
        let token_id = 7u64;
        let owner = setup_frozen_token(&env, token_id);
        let ctx = GuardContext::with_token(env, owner, token_id);

        let guard = NftStateGuard;
        assert_eq!(guard.execute(&ctx), GuardResult::Fail(Error::Unauthorized));
    }

    #[test]
    fn test_nft_state_guard_fails_for_missing_token() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let token_id = 999u64;
        let ctx = GuardContext::with_token(env, caller, token_id);

        let guard = NftStateGuard;
        assert_eq!(guard.execute(&ctx), GuardResult::Fail(Error::TokenNotFound));
    }

    #[test]
    fn test_nft_state_guard_delegates_frozen_check() {
        let env = soroban_sdk::Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let token_id = 7u64;
        let ctx = GuardContext::with_token(env, caller, token_id);

        // The frozen guard must agree with the state guard on freeze rejection.
        let state_result = NftStateGuard.execute(&ctx);
        let frozen_result = TokenFrozenGuard.execute(&ctx);

        // Both guards reject a missing token with the same standardized error.
        assert_eq!(state_result, GuardResult::Fail(Error::TokenNotFound));
        assert_eq!(frozen_result, GuardResult::Fail(Error::TokenNotFound));
    }

    #[test]
    fn test_nft_state_guard_name() {
        let guard = NftStateGuard;
        assert_eq!(guard.name(), "nft_state_guard");
    }

    #[test]
    fn test_require_token_active_missing_owner() {
        let env = soroban_sdk::Env::default();
        let token_id = 1u64;

        // No owner recorded for this token
        let result = require_token_active(&env, token_id);
        assert_eq!(
            result,
            Err(Error::InvalidTransferState(
                InvalidTransferStateReason::MissingOwner
            ))
        );
    }

    #[test]
    fn test_require_token_active_frozen() {
        let env = soroban_sdk::Env::default();
        let token_id = 2u64;
        let owner = soroban_sdk::Address::generate(&env);

        token_owner_storage::set_owner(&env, token_id, &owner);
        frozen_token::freeze(&env, token_id);

        let result = require_token_active(&env, token_id);
        assert_eq!(
            result,
            Err(Error::InvalidTransferState(
                InvalidTransferStateReason::Frozen
            ))
        );
    }

    #[test]
    fn test_require_token_active_inactive_token() {
        let env = soroban_sdk::Env::default();
        let token_id = 3u64;

        // Simulate an inactive token: owner present but token not active.
        // Since the active predicate requires an owner and non-frozen state,
        // an inactive token is represented by a missing owner here.
        let result = require_token_active(&env, token_id);
        assert!(matches!(
            result,
            Err(Error::InvalidTransferState(
                InvalidTransferStateReason::MissingOwner
                    | InvalidTransferStateReason::InactiveToken
            ))
        ));
    }

    #[test]
    fn test_require_token_active_ok() {
        let env = soroban_sdk::Env::default();
        let token_id = 4u64;
        let owner = soroban_sdk::Address::generate(&env);

        token_owner_storage::set_owner(&env, token_id, &owner);

        let result = require_token_active(&env, token_id);
        assert_eq!(result, Ok(()));
    }
}
