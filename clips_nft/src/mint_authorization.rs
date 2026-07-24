//! Mint authorization — validates that the caller is authorized to mint NFTs.
//!
//! Supports multiple authorization models:
//! - Contract owner/administrator
//! - Approved minter role
//!
//! This module provides a single entry point for mint authorization checks.

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error};

/// Validate that the caller is authorized to mint NFTs.
///
/// Authorization is granted if the caller is:
/// 1. The contract owner/admin, OR
/// 2. An approved minter
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `caller` - The address attempting to mint
///
/// # Returns
/// * `Ok(())` - If the caller is authorized
/// * `Err(Error::UnauthorizedMinter)` - If the caller is not authorized
///
/// # Errors
/// - [`Error::NotInitialized`] if the contract has not been initialized
/// - [`Error::UnauthorizedMinter`] if the caller is not authorized
pub fn require_mint_auth(env: &Env, caller: &Address) -> Result<(), Error> {
    // Check if caller is the contract owner/admin
    if is_contract_owner(env, caller)? {
        caller.require_auth();
        return Ok(());
    }

    // Check if caller is an approved minter
    if is_approved_minter(env, caller) {
        caller.require_auth();
        return Ok(());
    }

    // Caller is not authorized
    Err(Error::UnauthorizedMinter)
}

/// Check if the address is the contract owner/admin.
///
/// # Returns
/// * `Ok(true)` - If the address is the contract owner
/// * `Ok(false)` - If the address is not the contract owner
/// * `Err(Error::NotInitialized)` - If the contract is not initialized
fn is_contract_owner(env: &Env, address: &Address) -> Result<bool, Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    Ok(*address == admin)
}

/// Check if the address is an approved minter.
fn is_approved_minter(env: &Env, address: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<DataKey, Address>(&DataKey::ApprovedMinter(address.clone()))
        .map(|approved| approved == *address)
        .unwrap_or(false)
}

/// Set an approved minter address.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `minter` - The address to approve as a minter
///
/// # Note
/// This should be called by the contract owner/admin only.
pub fn set_approved_minter(env: &Env, minter: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::ApprovedMinter(minter.clone()), minter);
}

/// Remove an approved minter address.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `minter` - The address to remove from approved minters
///
/// # Note
/// This should be called by the contract owner/admin only.
pub fn remove_approved_minter(env: &Env, minter: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::ApprovedMinter(minter.clone()));
}

/// Check if an address is an approved minter.
///
/// # Returns
/// `true` if the address is an approved minter, `false` otherwise
pub fn is_minter(env: &Env, address: &Address) -> bool {
    is_approved_minter(env, address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_contract_owner_is_authorized() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &owner);
        
        assert!(require_mint_auth(&env, &owner).is_ok());
    }

    #[test]
    fn test_non_owner_is_unauthorized_without_minter_role() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &owner);
        
        assert_eq!(require_mint_auth(&env, &other), Err(Error::UnauthorizedMinter));
    }

    #[test]
    fn test_approved_minter_is_authorized() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let minter = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &owner);
        set_approved_minter(&env, &minter);
        
        assert!(require_mint_auth(&env, &minter).is_ok());
    }

    #[test]
    fn test_remove_minter_revokes_authorization() {
        let env = Env::default();
        env.mock_all_auths();
        let owner = Address::generate(&env);
        let minter = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &owner);
        set_approved_minter(&env, &minter);
        remove_approved_minter(&env, &minter);
        
        assert_eq!(require_mint_auth(&env, &minter), Err(Error::UnauthorizedMinter));
    }

    #[test]
    fn test_is_minter_returns_true_for_approved() {
        let env = Env::default();
        let minter = Address::generate(&env);
        set_approved_minter(&env, &minter);
        
        assert!(is_minter(&env, &minter));
    }

    #[test]
    fn test_is_minter_returns_false_for_non_approved() {
        let env = Env::default();
        let minter = Address::generate(&env);
        
        assert!(!is_minter(&env, &minter));
    }
}