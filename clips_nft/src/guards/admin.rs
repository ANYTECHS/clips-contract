//! Admin authorization guard — restrict operations to administrators (issue #1091).
//!
//! This guard module re-exports the existing admin access control functionality
//! from [`crate::admin_access_control_guard`] as part of the centralized guards
//! framework.
//!
//! For detailed documentation, see [`crate::admin_access_control_guard`].

pub use crate::admin_access_control_guard::{
    check_caller_is_admin, get_configured_admin, require_admin,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataKey, Error};
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn require_admin_accepts_configured_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert!(require_admin(env, &admin).is_ok());
        });
    }

    #[test]
    fn require_admin_rejects_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert_eq!(require_admin(env, &other), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn check_caller_is_admin_returns_boolean() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert!(check_caller_is_admin(env, &admin));
            let other = Address::generate(env);
            assert!(!check_caller_is_admin(env, &other));
        });
    }

    #[test]
    fn get_configured_admin_returns_stored_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            let retrieved = get_configured_admin(env).unwrap();
            assert_eq!(retrieved, admin);
        });
    }
}
