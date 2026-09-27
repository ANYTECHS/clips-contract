//! Initialization guard (Issues #484, #1075).
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

use soroban_sdk::{Address, Env};

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

/// Guard for operations that need the contract to be initialized.
///
/// # Errors
/// - [`Error::NotInitialized`] if [`DataKey::Admin`] is not yet stored.
pub fn require_initialized(env: &Env) -> Result<(), Error> {
    if !is_initialized(env) {
        return Err(Error::NotInitialized);
    }
    Ok(())
}

/// Run first-time initialization exactly once.
///
/// Rejects the call if the contract is already initialized, otherwise runs
/// `init` and then records `admin` under [`DataKey::Admin`], which marks the
/// contract as initialized. If `init` fails, nothing is recorded and a later
/// call may retry.
///
/// # Errors
/// - [`Error::AlreadyInitialized`] on any repeated initialization.
/// - Any error returned by `init`.
pub fn initialize_once<F>(env: &Env, admin: &Address, init: F) -> Result<(), Error>
where
    F: FnOnce() -> Result<(), Error>,
{
    require_not_initialized(env)?;
    init()?;
    env.storage().instance().set(&DataKey::Admin, admin);
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

    // ─── require_initialized / initialize_once (#1075) ───────────────────────

    fn contract_env() -> (Env, Address) {
        let env = Env::default();
        let id = env.register(crate::ClipCashNFT, ());
        (env, id)
    }

    #[test]
    fn require_initialized_rejects_before_init() {
        let (env, id) = contract_env();
        env.as_contract(&id, || {
            assert_eq!(require_initialized(&env), Err(Error::NotInitialized));
        });
    }

    #[test]
    fn initialize_once_allows_first_initialization() {
        let (env, id) = contract_env();
        env.as_contract(&id, || {
            let admin = Address::generate(&env);
            assert_eq!(initialize_once(&env, &admin, || Ok(())), Ok(()));
            assert!(is_initialized(&env));
            assert_eq!(require_initialized(&env), Ok(()));
            let stored: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
            assert_eq!(stored, admin);
        });
    }

    #[test]
    fn initialize_once_rejects_repeat_and_skips_init_logic() {
        let (env, id) = contract_env();
        env.as_contract(&id, || {
            let admin = Address::generate(&env);
            let attacker = Address::generate(&env);
            initialize_once(&env, &admin, || Ok(())).unwrap();

            let mut ran = false;
            let result = initialize_once(&env, &attacker, || {
                ran = true;
                Ok(())
            });
            assert_eq!(result, Err(Error::AlreadyInitialized));
            assert!(!ran);
            let stored: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
            assert_eq!(stored, admin);
        });
    }

    #[test]
    fn failed_initialization_does_not_mark_initialized() {
        let (env, id) = contract_env();
        env.as_contract(&id, || {
            let admin = Address::generate(&env);
            let result = initialize_once(&env, &admin, || Err(Error::InvalidConfig));
            assert_eq!(result, Err(Error::InvalidConfig));
            assert!(!is_initialized(&env));
            assert_eq!(initialize_once(&env, &admin, || Ok(())), Ok(()));
        });
    }
}
