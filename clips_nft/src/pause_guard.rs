//! Pause guard.
//!
//! Returns [`Error::ContractPaused`] when the contract is paused,
//! preventing execution of state-changing functions.

use soroban_sdk::Env;

use crate::pause_state::get_pause_state;
use crate::types::Error;

/// Reject the current invocation if the contract is paused.
///
/// Call this at the top of every state-changing function.
///
/// # Errors
/// Returns [`Error::ContractPaused`] if paused.
pub fn require_not_paused(env: &Env) -> Result<(), Error> {
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
    fn require_not_passed_when_not_paused() {
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
}
