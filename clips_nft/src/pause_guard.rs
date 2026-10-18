//! Pause Guard (Issue #1073).
//!
//! Prevents protected contract operations from executing while the contract is
//! paused.  Emergency operations that must remain accessible during a pause
//! (e.g. admin un-pause) are explicitly permitted via [`check_emergency_allowed`].
//!
//! # Guard functions
//!
//! | Function | Purpose |
//! |----------|---------|
//! | [`require_not_paused`] | Reject any state-changing call while paused. |
//! | [`require_paused`] | Reject calls that only make sense while paused (e.g. `unpause`). |
//! | [`check_emergency_allowed`] | Allow an explicit set of emergency operations through even when paused. |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Normal protected operation — must fail while paused.
//! pause_guard::require_not_paused(&env)?;
//!
//! // Emergency operation — always allowed regardless of pause state.
//! pause_guard::check_emergency_allowed(&env, operation, &PERMITTED_OPS)?;
//! ```

use soroban_sdk::Env;

use crate::pause_state::{get_pause_state, save_pause_state};
use crate::types::Error;

// ─── Emergency operation identifiers ─────────────────────────────────────────

/// Symbolic constant for the "unpause" emergency operation.
///
/// Pass this to [`check_emergency_allowed`] to express that the unpause
/// entry-point is always permitted even while paused.
pub const OP_UNPAUSE: u32 = 1;

/// Symbolic constant for an admin-override emergency operation.
pub const OP_ADMIN_OVERRIDE: u32 = 2;

// ─── Guard functions ──────────────────────────────────────────────────────────

/// Reject the current invocation if the contract is paused.
///
/// Call this at the top of every state-changing function to prevent
/// execution while the contract is in the paused state.
///
/// # Errors
/// Returns [`Error::ContractPaused`] if the contract is paused.
pub fn require_not_paused(env: &Env) -> Result<(), Error> {
    if get_pause_state(env) {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

/// Reject the current invocation if the contract is **not** paused.
///
/// Use this as the guard for operations that are only valid when the contract
/// is already paused — for example, an `unpause` entry point.
///
/// # Errors
/// Returns [`Error::NotPaused`] if the contract is not currently paused.
pub fn require_paused(env: &Env) -> Result<(), Error> {
    if !get_pause_state(env) {
        return Err(Error::NotPaused);
    }
    Ok(())
}

/// Allow an explicitly-listed emergency operation through regardless of pause.
///
/// Certain operations (e.g. un-pausing, admin overrides) must remain
/// executable even while the contract is paused.  Pass the numeric identifier
/// of the requested operation and the slice of permitted identifiers; returns
/// `Ok(())` if the operation is on the allow-list, otherwise falls through to
/// the normal not-paused check.
///
/// # Arguments
/// * `env`         — Contract environment.
/// * `operation`   — Identifier of the operation being attempted.
/// * `allowed_ops` — Slice of operation identifiers always permitted.
///
/// # Errors
/// Returns [`Error::ContractPaused`] when the operation is **not** in
/// `allowed_ops` and the contract is currently paused.
pub fn check_emergency_allowed(
    env: &Env,
    operation: u32,
    allowed_ops: &[u32],
) -> Result<(), Error> {
    if allowed_ops.contains(&operation) {
        return Ok(());
    }
    require_not_paused(env)
}

// ─── Convenience helpers ──────────────────────────────────────────────────────

/// Pause the contract.
///
/// Callers are responsible for verifying admin identity before invoking this.
pub fn pause(env: &Env) {
    save_pause_state(env, true);
}

/// Unpause the contract.
///
/// Callers are responsible for verifying admin identity before invoking this.
pub fn unpause(env: &Env) {
    save_pause_state(env, false);
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause_state::save_pause_state;
    use soroban_sdk::Env;

    // ── require_not_paused ────────────────────────────────────────────────────

    #[test]
    fn require_not_paused_passes_when_not_paused() {
        let env = Env::default();
        save_pause_state(&env, false);
        assert!(require_not_paused(&env).is_ok());
    }

    #[test]
    fn require_not_paused_returns_error_when_paused() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert_eq!(require_not_paused(&env), Err(Error::ContractPaused));
    }

    #[test]
    fn require_not_paused_passes_after_contract_unpaused() {
        let env = Env::default();
        save_pause_state(&env, true);
        save_pause_state(&env, false);
        assert!(require_not_paused(&env).is_ok());
    }

    // ── require_paused ────────────────────────────────────────────────────────

    #[test]
    fn require_paused_passes_when_contract_is_paused() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert!(require_paused(&env).is_ok());
    }

    #[test]
    fn require_paused_returns_not_paused_when_contract_is_active() {
        let env = Env::default();
        save_pause_state(&env, false);
        assert_eq!(require_paused(&env), Err(Error::NotPaused));
    }

    // ── check_emergency_allowed ───────────────────────────────────────────────

    #[test]
    fn emergency_op_is_permitted_while_paused() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert!(check_emergency_allowed(&env, OP_UNPAUSE, &[OP_UNPAUSE]).is_ok());
    }

    #[test]
    fn non_emergency_op_is_blocked_while_paused() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert_eq!(
            check_emergency_allowed(&env, 99, &[OP_UNPAUSE]),
            Err(Error::ContractPaused)
        );
    }

    #[test]
    fn any_op_passes_when_contract_is_not_paused() {
        let env = Env::default();
        save_pause_state(&env, false);
        assert!(check_emergency_allowed(&env, 99, &[]).is_ok());
    }

    #[test]
    fn multiple_emergency_ops_are_all_permitted() {
        let env = Env::default();
        save_pause_state(&env, true);
        let allowed = [OP_UNPAUSE, OP_ADMIN_OVERRIDE];
        assert!(check_emergency_allowed(&env, OP_UNPAUSE, &allowed).is_ok());
        assert!(check_emergency_allowed(&env, OP_ADMIN_OVERRIDE, &allowed).is_ok());
    }

    #[test]
    fn unlisted_op_is_blocked_with_multiple_emergency_ops_configured() {
        let env = Env::default();
        save_pause_state(&env, true);
        let allowed = [OP_UNPAUSE, OP_ADMIN_OVERRIDE];
        assert_eq!(
            check_emergency_allowed(&env, 42, &allowed),
            Err(Error::ContractPaused)
        );
    }

    // ── pause / unpause helpers ───────────────────────────────────────────────

    #[test]
    fn pause_sets_contract_to_paused_state() {
        let env = Env::default();
        pause(&env);
        assert_eq!(require_not_paused(&env), Err(Error::ContractPaused));
    }

    #[test]
    fn unpause_clears_paused_state() {
        let env = Env::default();
        pause(&env);
        unpause(&env);
        assert!(require_not_paused(&env).is_ok());
    }
}
