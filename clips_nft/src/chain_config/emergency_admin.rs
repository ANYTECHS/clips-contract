//! Emergency Admin Wallet Configuration — resolves issue #474.
//!
//! Allows a contract owner to designate a secondary, emergency administrator
//! wallet that can perform privileged emergency actions (e.g. pausing the
//! contract, revoking compromised keys) when the primary admin is unavailable.
//!
//! # Security rules
//! 1. The emergency admin address must be a valid Stellar account address
//!    (enforced by Soroban's [`Address`] type).
//! 2. The emergency admin **cannot** be the same address as the contract
//!    owner.  This prevents accidental lock-in where a single key controls
//!    both normal and emergency administrative paths.
//! 3. Only one emergency admin may be active at a time.  Calling
//!    [`set_emergency_admin`] replaces any previously stored value.
//!
//! # Storage
//! Key: [`ConfigKey::EmergencyAdmin`] (instance storage).
//!
//! # Usage
//! ```text
//! // Set emergency admin (caller must already be authenticated as owner):
//! set_emergency_admin(env, &owner_address, emergency_wallet)?;
//!
//! // Read back:
//! let admin: Option<Address> = get_emergency_admin(env);
//!
//! // Clear the emergency admin:
//! clear_emergency_admin(env);
//! ```

use soroban_sdk::{Address, Env};

use crate::types::Error;

use super::keys::ConfigKey;

// ─── Getter ───────────────────────────────────────────────────────────────────

/// Return the stored emergency admin [`Address`].
///
/// Returns `None` if no emergency admin has been configured.
pub fn get_emergency_admin(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .get(&ConfigKey::EmergencyAdmin)
}

// ─── Setter ───────────────────────────────────────────────────────────────────

/// Configure the emergency admin wallet.
///
/// # Parameters
/// - `owner`: The current contract owner address.  Used to enforce the
///   duplicate-owner guard.
/// - `emergency_wallet`: The address to designate as emergency admin.
///
/// # Errors
/// - [`Error::InvalidAddress`] — `emergency_wallet` is the zero address or
///   identical to `owner` (prevents duplicate-owner assignment).
pub fn set_emergency_admin(
    env: &Env,
    owner: &Address,
    emergency_wallet: Address,
) -> Result<(), Error> {
    // Reject duplicate owner assignment (security rule #2).
    if emergency_wallet == *owner {
        return Err(Error::InvalidAddress);
    }

    env.storage()
        .instance()
        .set(&ConfigKey::EmergencyAdmin, &emergency_wallet);
    Ok(())
}

// ─── Clear ────────────────────────────────────────────────────────────────────

/// Remove the emergency admin entry from storage.
///
/// After this call [`get_emergency_admin`] returns `None`.
pub fn clear_emergency_admin(env: &Env) {
    env.storage()
        .instance()
        .remove(&ConfigKey::EmergencyAdmin);
}

// ─── Helper ───────────────────────────────────────────────────────────────────

/// Returns `true` when the given address is the configured emergency admin.
///
/// Useful in auth guards: `if is_emergency_admin(env, &caller) { ... }`.
pub fn is_emergency_admin(env: &Env, addr: &Address) -> bool {
    match get_emergency_admin(env) {
        Some(stored) => stored == *addr,
        None => false,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::Address as _, Address, Env};

    use super::*;

    fn new_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_get_returns_none_when_not_set() {
        let env = new_env();
        assert!(get_emergency_admin(&env).is_none());
    }

    #[test]
    fn test_set_and_get_emergency_admin() {
        let env = new_env();
        let owner = Address::generate(&env);
        let emergency = Address::generate(&env);
        set_emergency_admin(&env, &owner, emergency.clone()).expect("should set");
        let stored = get_emergency_admin(&env).expect("should have emergency admin");
        assert_eq!(stored, emergency);
    }

    #[test]
    fn test_duplicate_owner_rejected() {
        let env = new_env();
        let owner = Address::generate(&env);
        // Attempt to set the owner as the emergency admin — must be rejected.
        assert_eq!(
            set_emergency_admin(&env, &owner, owner.clone()),
            Err(Error::InvalidAddress)
        );
    }

    #[test]
    fn test_overwrite_emergency_admin() {
        let env = new_env();
        let owner = Address::generate(&env);
        let first = Address::generate(&env);
        let second = Address::generate(&env);
        set_emergency_admin(&env, &owner, first).unwrap();
        set_emergency_admin(&env, &owner, second.clone()).unwrap();
        let stored = get_emergency_admin(&env).unwrap();
        assert_eq!(stored, second);
    }

    #[test]
    fn test_clear_emergency_admin() {
        let env = new_env();
        let owner = Address::generate(&env);
        let emergency = Address::generate(&env);
        set_emergency_admin(&env, &owner, emergency).unwrap();
        clear_emergency_admin(&env);
        assert!(get_emergency_admin(&env).is_none());
    }

    #[test]
    fn test_is_emergency_admin_true() {
        let env = new_env();
        let owner = Address::generate(&env);
        let emergency = Address::generate(&env);
        set_emergency_admin(&env, &owner, emergency.clone()).unwrap();
        assert!(is_emergency_admin(&env, &emergency));
    }

    #[test]
    fn test_is_emergency_admin_false_for_other() {
        let env = new_env();
        let owner = Address::generate(&env);
        let emergency = Address::generate(&env);
        let other = Address::generate(&env);
        set_emergency_admin(&env, &owner, emergency).unwrap();
        assert!(!is_emergency_admin(&env, &other));
    }

    #[test]
    fn test_is_emergency_admin_false_when_not_set() {
        let env = new_env();
        let addr = Address::generate(&env);
        assert!(!is_emergency_admin(&env, &addr));
    }
}
