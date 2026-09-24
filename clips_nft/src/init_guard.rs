//! Initialization guard (Issue #484).
//!
//! Provides a reusable guard that prevents contract initialization logic
//! from being executed more than once.
//!
//! # Two-tier design
//!
//! - [`is_initialized`] — a boolean check (no panic, no auth).
//! - [`require_not_initialized`] — the full guard: rejects repeated
//!   initialization, allows valid first-time initialization.
//!
//! # Usage
//!
//! ```rust,ignore
//! init_guard::require_not_initialized(&env)?;
//! // If Ok, the contract is not yet initialized — safe to proceed.
//! ```
//!
//! # Storage
//!
//! Initialization is detected by checking the presence of
//! [`DataKey::Admin`] in instance storage.  This key is written exactly
//! once, during the first successful `init` call, and is never removed.

use soroban_sdk::Env;

use crate::types::{DataKey, Error};

/// Return `true` when the contract has already been initialized.
///
/// Initialization is determined by the presence of the [`DataKey::Admin`]
/// record in instance storage.
pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

/// Guard that rejects repeated initialization.
///
/// # Errors
/// - [`Error::AlreadyInitialized`] if the contract has already been
///   initialized (i.e. [`DataKey::Admin`] is present).
pub fn require_not_initialized(env: &Env) -> Result<(), Error> {
    if is_initialized(env) {
        return Err(Error::AlreadyInitialized);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DataKey;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn set_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    // ─── is_initialized ───────────────────────────────────────────────────────

    #[test]
    fn is_initialized_returns_false_before_init() {
        let env = Env::default();
        assert!(!is_initialized(&env));
    }

    #[test]
    fn is_initialized_returns_true_after_admin_set() {
        let env = Env::default();
        let admin = Address::generate(&env);
        set_admin(&env, &admin);
        assert!(is_initialized(&env));
    }

    // ─── require_not_initialized ─────────────────────────────────────────────

    #[test]
    fn require_not_initialized_passes_before_init() {
        let env = Env::default();
        assert!(require_not_initialized(&env).is_ok());
    }

    #[test]
    fn require_not_initialized_rejects_after_init() {
        let env = Env::default();
        let admin = Address::generate(&env);
        set_admin(&env, &admin);

        assert_eq!(
            require_not_initialized(&env),
            Err(Error::AlreadyInitialized)
        );
    }

    // ─── Idempotency ─────────────────────────────────────────────────────────

    #[test]
    fn second_require_not_initialized_call_also_rejects() {
        let env = Env::default();
        let admin = Address::generate(&env);
        set_admin(&env, &admin);

        let first = require_not_initialized(&env);
        let second = require_not_initialized(&env);
        assert_eq!(first, Err(Error::AlreadyInitialized));
        assert_eq!(second, Err(Error::AlreadyInitialized));
    }
}
