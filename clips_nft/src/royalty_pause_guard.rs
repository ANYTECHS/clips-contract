//! Royalty pause validation — ensures royalty state-changing operations
//! respect the global contract pause state.
//!
//! When the contract is paused, royalty configuration updates, payment
//! processing, and recipient changes must all be blocked. This guard
//! reuses the existing pause state infrastructure and provides a
//! domain-specific entry point for the royalty module.
//!
//! # Usage
//!
//! ```rust,ignore
//! royalty_pause_guard::require_royalty_not_paused(env)?;
//! ```

use crate::pause_state::get_pause_state;
use crate::types::Error;
use soroban_sdk::Env;

/// Reject the call if the contract is currently paused.
///
/// This is the royalty-specific pause check. It reads the same global
/// pause flag used by [`crate::pause_guard`] but is意图意图 to be called
/// at the top of every royalty state-changing function.
///
/// # Errors
/// Returns [`Error::ContractPaused`] if the contract is paused.
pub fn require_royalty_not_paused(env: &Env) -> Result<(), Error> {
    if get_pause_state(env) {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause_state::save_pause_state;
    use soroban_sdk::Env;

    #[test]
    fn passes_when_not_paused() {
        let env = Env::default();
        save_pause_state(&env, false);
        assert!(require_royalty_not_paused(&env).is_ok());
    }

    #[test]
    fn returns_error_when_paused() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert_eq!(require_royalty_not_paused(&env), Err(Error::ContractPaused));
    }

    #[test]
    fn passes_after_unpause() {
        let env = Env::default();
        save_pause_state(&env, true);
        save_pause_state(&env, false);
        assert!(require_royalty_not_paused(&env).is_ok());
    }
}
