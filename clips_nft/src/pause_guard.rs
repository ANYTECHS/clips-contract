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
