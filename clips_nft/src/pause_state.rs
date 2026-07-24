//! Pause state storage.
//!
//! Persists whether the contract is paused.
//!
//! # Storage
//! Key: [`DataKey::Paused`] (instance storage).

use soroban_sdk::Env;

use crate::types::DataKey;

/// Persist the pause state.
pub fn save_pause_state(env: &Env, paused: bool) {
    env.storage()
        .instance()
        .set(&DataKey::Paused, &paused);
}

/// Return the current pause state (`false` if never set).
pub fn get_pause_state(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn get_pause_state_defaults_to_false() {
        let env = Env::default();
        assert!(!get_pause_state(&env));
    }

    #[test]
    fn save_and_get_pause_state_true() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert!(get_pause_state(&env));
    }

    #[test]
    fn save_and_get_pause_state_false() {
        let env = Env::default();
        save_pause_state(&env, false);
        assert!(!get_pause_state(&env));
    }

    #[test]
    fn save_pause_state_overrides_previous() {
        let env = Env::default();
        save_pause_state(&env, true);
        assert!(get_pause_state(&env));
        save_pause_state(&env, false);
        assert!(!get_pause_state(&env));
    }
}
