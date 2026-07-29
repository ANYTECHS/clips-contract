//! Approved administrator storage.
//!
//! Maintains the set of wallet addresses that are authorised as administrators.
//! Each address is stored independently in persistent storage so approvals can
//! be granted or revoked per-account.
//!
//! # Storage
//! Key: `DataKey::Administrator(admin)` (persistent storage)
//! Value: `bool` — `true` means the address is currently an admin.

use soroban_sdk::{Address, Env};

use crate::types::DataKey;

/// Add an administrator account.
///
/// Calling this function when `admin` is already an administrator is a no-op.
pub fn add_admin(env: &Env, admin: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Administrator(admin.clone()), &true);
}

/// Remove an administrator account.
///
/// If `admin` was never an administrator, this function is a no-op.
pub fn remove_admin(env: &Env, admin: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Administrator(admin.clone()));
}

/// Check if the address is currently an administrator.
pub fn is_admin(env: &Env, admin: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Administrator(admin.clone()))
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn test_env() -> Env {
        Env::default()
    }

    #[test]
    fn add_then_query_returns_true() {
        let env = test_env();
        let admin = Address::generate(&env);

        add_admin(&env, &admin);

        assert!(is_admin(&env, &admin));
    }

    #[test]
    fn remove_then_query_returns_false() {
        let env = test_env();
        let admin = Address::generate(&env);

        add_admin(&env, &admin);
        remove_admin(&env, &admin);

        assert!(!is_admin(&env, &admin));
    }

    #[test]
    fn query_before_add_returns_false() {
        let env = test_env();
        let admin = Address::generate(&env);

        assert!(!is_admin(&env, &admin));
    }

    #[test]
    fn add_same_admin_twice_is_idempotent() {
        let env = test_env();
        let admin = Address::generate(&env);

        add_admin(&env, &admin);
        add_admin(&env, &admin);

        assert!(is_admin(&env, &admin));
    }

    #[test]
    fn remove_non_existent_admin_is_noop() {
        let env = test_env();
        let admin = Address::generate(&env);

        remove_admin(&env, &admin);

        assert!(!is_admin(&env, &admin));
    }

    #[test]
    fn multiple_distinct_admins_stored_independently() {
        let env = test_env();
        let admin_a = Address::generate(&env);
        let admin_b = Address::generate(&env);
        let admin_c = Address::generate(&env);

        add_admin(&env, &admin_a);
        add_admin(&env, &admin_b);

        assert!(is_admin(&env, &admin_a));
        assert!(is_admin(&env, &admin_b));
        assert!(!is_admin(&env, &admin_c));

        remove_admin(&env, &admin_b);

        assert!(is_admin(&env, &admin_a));
        assert!(!is_admin(&env, &admin_b));
        assert!(!is_admin(&env, &admin_c));
    }
}
