//! Administrator account storage.
//!
//! Maintains the set of wallet addresses authorised as administrators.
//! Each address is stored independently in persistent storage so approvals
//! can be granted or revoked per-account without touching other admins.
//!
//! # Storage
//! Key:    [`StorageKey::Administrator(addr)`] (persistent storage)
//! Value:  `bool` — `true` means the address is a current administrator.
//!
//! # Operations
//! - [`add_admin`]    — grant administrator status
//! - [`remove_admin`] — revoke administrator status
//! - [`is_admin`]     — query whether an address is an administrator

use soroban_sdk::{Address, Env};

use crate::storage::keys::StorageKey;

/// Grant administrator status to `admin`.
///
/// Calling this when `admin` is already an administrator is a no-op.
pub fn add_admin(env: &Env, admin: &Address) {
    env.storage()
        .persistent()
        .set(&StorageKey::Administrator(admin.clone()), &true);
}

/// Revoke administrator status from `admin`.
///
/// If `admin` was never an administrator this function is a no-op.
pub fn remove_admin(env: &Env, admin: &Address) {
    env.storage()
        .persistent()
        .remove(&StorageKey::Administrator(admin.clone()));
}

/// Return `true` if `admin` currently holds administrator status.
pub fn is_admin(env: &Env, admin: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&StorageKey::Administrator(admin.clone()))
        .unwrap_or(false)
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let id = env.register(AtomicMintContract, ());
        env.as_contract(&id, || f(&env))
    }
    #[ignore]
    #[test]
    fn add_then_query_returns_true() {
        with_contract(|env| {
            let admin = Address::generate(env);
            add_admin(env, &admin);
            assert!(is_admin(env, &admin));
        });
    }
    #[ignore]
    #[test]
    fn remove_then_query_returns_false() {
        with_contract(|env| {
            let admin = Address::generate(env);
            add_admin(env, &admin);
            remove_admin(env, &admin);
            assert!(!is_admin(env, &admin));
        });
    }
    #[ignore]
    #[test]
    fn query_before_add_returns_false() {
        with_contract(|env| {
            let admin = Address::generate(env);
            assert!(!is_admin(env, &admin));
        });
    }
    #[ignore]
    #[test]
    fn add_same_admin_twice_is_idempotent() {
        with_contract(|env| {
            let admin = Address::generate(env);
            add_admin(env, &admin);
            add_admin(env, &admin);
            assert!(is_admin(env, &admin));
        });
    }
    #[ignore]
    #[test]
    fn remove_non_existent_is_noop() {
        with_contract(|env| {
            let admin = Address::generate(env);
            remove_admin(env, &admin); // should not panic
            assert!(!is_admin(env, &admin));
        });
    }
    #[ignore]
    #[test]
    fn multiple_admins_stored_independently() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let c = Address::generate(env);

            add_admin(env, &a);
            add_admin(env, &b);

            assert!(is_admin(env, &a));
            assert!(is_admin(env, &b));
            assert!(!is_admin(env, &c));

            remove_admin(env, &b);

            assert!(is_admin(env, &a));
            assert!(!is_admin(env, &b));
        });
    }
}
