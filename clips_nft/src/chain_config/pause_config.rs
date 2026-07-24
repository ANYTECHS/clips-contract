//! Contract Pause Configuration — resolves issue #475.
//!
//! Stores the contract pause flag inside the configuration module,
//! providing a single, authoritative location for pause state management.
//!
//! # Behaviour
//! When `paused` is `true`:
//! - Mint operations **must** be rejected by callers that check this flag.
//! - Transfer operations **must** be rejected by callers that check this flag.
//! - Administrative read/write operations (e.g. config updates) are **not**
//!   automatically blocked; each admin endpoint decides independently.
//!
//! When `paused` is `false` (the default):
//! - All operations proceed normally.
//!
//! # Storage
//! Key: [`ConfigKey::Paused`] (instance storage).
//! Instance storage is used so the pause flag is always accessible in a
//! single ledger read, regardless of token count.
//!
//! # Usage
//! ```text
//! // Pause the contract (admin only — auth enforced by caller):
//! set_paused(env, true);
//!
//! // Unpause:
//! set_paused(env, false);
//!
//! // Guard an operation:
//! if is_paused(env) {
//!     return Err(Error::ContractPaused);
//! }
//! ```

use soroban_sdk::Env;

use crate::types::Error;

use super::keys::ConfigKey;

// ─── Getter ───────────────────────────────────────────────────────────────────

/// Return `true` when the contract is paused, `false` otherwise.
///
/// Defaults to `false` when the key has never been written (i.e. the contract
/// is active by default until an admin explicitly pauses it).
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&ConfigKey::Paused)
        .unwrap_or(false)
}

// ─── Setter ───────────────────────────────────────────────────────────────────

/// Set the contract pause state.
///
/// Pass `true` to pause the contract and `false` to resume normal operation.
///
/// # Errors
/// Returns [`Error::ContractPaused`] when attempting to pause a contract that
/// is already paused, and [`Error::NotPaused`] when attempting to unpause a
/// contract that is not paused. This prevents redundant no-op state changes
/// and ensures callers receive clear feedback about the current state.
pub fn set_paused(env: &Env, paused: bool) -> Result<(), Error> {
    let current = is_paused(env);
    if paused && current {
        return Err(Error::ContractPaused);
    }
    if !paused && !current {
        return Err(Error::NotPaused);
    }
    env.storage()
        .instance()
        .set(&ConfigKey::Paused, &paused);
    Ok(())
}

/// Unconditionally write the pause state without guard checks.
///
/// Intended for use during contract initialisation where the initial state
/// must be set without triggering the idempotency guards in [`set_paused`].
pub fn init_paused(env: &Env, paused: bool) {
    env.storage()
        .instance()
        .set(&ConfigKey::Paused, &paused);
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use soroban_sdk::Env;

    use super::*;

    fn new_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_default_is_not_paused() {
        let env = new_env();
        assert!(!is_paused(&env));
    }

    #[test]
    fn test_pause_contract() {
        let env = new_env();
        assert!(!is_paused(&env));
        set_paused(&env, true).expect("should pause");
        assert!(is_paused(&env));
    }

    #[test]
    fn test_unpause_contract() {
        let env = new_env();
        set_paused(&env, true).unwrap();
        set_paused(&env, false).expect("should unpause");
        assert!(!is_paused(&env));
    }

    #[test]
    fn test_pause_already_paused_returns_error() {
        let env = new_env();
        set_paused(&env, true).unwrap();
        assert_eq!(set_paused(&env, true), Err(Error::ContractPaused));
    }

    #[test]
    fn test_unpause_when_not_paused_returns_error() {
        let env = new_env();
        assert_eq!(set_paused(&env, false), Err(Error::NotPaused));
    }

    #[test]
    fn test_init_paused_true() {
        let env = new_env();
        init_paused(&env, true);
        assert!(is_paused(&env));
    }

    #[test]
    fn test_init_paused_false() {
        let env = new_env();
        init_paused(&env, true);
        init_paused(&env, false);
        assert!(!is_paused(&env));
    }

    #[test]
    fn test_pause_unpause_cycle() {
        let env = new_env();
        // Pause
        set_paused(&env, true).unwrap();
        assert!(is_paused(&env));
        // Unpause
        set_paused(&env, false).unwrap();
        assert!(!is_paused(&env));
        // Re-pause
        set_paused(&env, true).unwrap();
        assert!(is_paused(&env));
    }
}
