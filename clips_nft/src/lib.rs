#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

// ============================================================================
// StorageKey Enum - Centralized definition of all storage keys (#491)
// ============================================================================
#[derive(Clone, Copy)]
#[contracttype]
pub enum StorageKey {
    /// Storage key for contract owner address
    Owner = 0,
    /// Storage key for administrator set
    Admin = 1,
    /// Storage key for default royalty percentage (in basis points)
    RoyaltyBps = 2,
}

// ============================================================================
// Owner Storage - Persistent storage for contract owner (#493)
// ============================================================================

/// Save the contract owner
fn save_owner(env: &Env, owner: &Address) {
    env.storage().persistent().set(&StorageKey::Owner, owner);
}

/// Read the contract owner
fn read_owner(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get::<StorageKey, Address>(&StorageKey::Owner)
        .unwrap_or_else(|| panic_with_error(env, "Owner not initialized"))
}

/// Update the contract owner
fn update_owner(env: &Env, new_owner: &Address) {
    save_owner(env, new_owner);
}

// ============================================================================
// Administrator Storage - Support for multiple admins (#494)
// ============================================================================

/// Add an administrator
fn add_admin(env: &Env, admin: &Address) {
    let mut admins = read_admins(env);
    
    // Prevent duplicate admins
    if !admins.contains(admin) {
        admins.push_back(admin.clone());
        env.storage().persistent().set(&StorageKey::Admin, &admins);
    }
}

/// Remove an administrator
fn remove_admin(env: &Env, admin: &Address) {
    let mut admins = read_admins(env);
    
    if let Some(pos) = admins.iter().position(|a| a == admin) {
        admins.remove(pos);
        env.storage().persistent().set(&StorageKey::Admin, &admins);
    }
}

/// Check if an address is an administrator
fn is_admin(env: &Env, admin: &Address) -> bool {
    let admins = read_admins(env);
    admins.contains(admin)
}

/// Read all administrators
fn read_admins(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get::<StorageKey, Vec<Address>>(&StorageKey::Admin)
        .unwrap_or_else(|| Vec::new(env))
}

// ============================================================================
// Royalty Configuration - Default royalty percentage (#469)
// ============================================================================

const MAX_BPS: u32 = 10_000; // 100% in basis points

/// Save the default royalty percentage (in basis points)
fn save_royalty_bps(env: &Env, bps: u32) {
    validate_bps(bps);
    env.storage().persistent().set(&StorageKey::RoyaltyBps, &bps);
}

/// Read the default royalty percentage (in basis points)
fn read_royalty_bps(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get::<StorageKey, u32>(&StorageKey::RoyaltyBps)
        .unwrap_or(0)
}

/// Validate BPS range (0 to 10000, representing 0% to 100%)
fn validate_bps(bps: u32) {
    if bps > MAX_BPS {
        panic!("Invalid BPS: must be between 0 and {}", MAX_BPS);
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

fn panic_with_error(env: &Env, msg: &str) -> ! {
    env.panic_with_error(msg.into());
}

// ============================================================================
// Main Contract
// ============================================================================

#[contract]
pub struct ClipsNftContract;

#[contractimpl]
impl ClipsNftContract {
    /// Initialize the contract with owner and default royalty
    pub fn initialize(env: Env, owner: Address, royalty_bps: u32) {
        owner.require_auth();
        
        // Validate royalty BPS
        validate_bps(royalty_bps);
        
        // Initialize owner
        save_owner(&env, &owner);
        
        // Add owner as initial admin
        add_admin(&env, &owner);
        
        // Set default royalty
        save_royalty_bps(&env, royalty_bps);
    }

    // ========================================================================
    // Owner Management (#493)
    // ========================================================================

    /// Get the current contract owner
    pub fn get_owner(env: Env) -> Address {
        read_owner(&env)
    }

    /// Set a new contract owner (only callable by current owner)
    pub fn set_owner(env: Env, new_owner: Address) {
        let current_owner = read_owner(&env);
        current_owner.require_auth();
        update_owner(&env, &new_owner);
    }

    // ========================================================================
    // Administrator Management (#494)
    // ========================================================================

    /// Add an administrator account
    pub fn add_administrator(env: Env, admin: Address) {
        let owner = read_owner(&env);
        owner.require_auth();
        add_admin(&env, &admin);
    }

    /// Remove an administrator account
    pub fn remove_administrator(env: Env, admin: Address) {
        let owner = read_owner(&env);
        owner.require_auth();
        remove_admin(&env, &admin);
    }

    /// Check if an address is an administrator
    pub fn is_administrator(env: Env, admin: Address) -> bool {
        is_admin(&env, &admin)
    }

    // ========================================================================
    // Royalty Configuration (#469)
    // ========================================================================

    /// Get the default royalty percentage in basis points
    pub fn get_royalty_bps(env: Env) -> u32 {
        read_royalty_bps(&env)
    }

    /// Set the default royalty percentage in basis points
    /// Only callable by owner. Must be between 0 and 10000 (0% to 100%)
    pub fn set_royalty_bps(env: Env, bps: u32) {
        let owner = read_owner(&env);
        owner.require_auth();
        save_royalty_bps(&env, bps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as AddressTestUtils;

    #[test]
    fn test_owner_management() {
        let env = soroban_sdk::Env::default();
        let owner = soroban_sdk::Address::generate(&env);

        ClipsNftContract::initialize(env.clone(), owner.clone(), 500);

        let retrieved_owner = ClipsNftContract::get_owner(env.clone());
        assert_eq!(retrieved_owner, owner);

        let new_owner = soroban_sdk::Address::generate(&env);
        ClipsNftContract::set_owner(env.clone(), new_owner.clone());
        let retrieved_owner = ClipsNftContract::get_owner(env.clone());
        assert_eq!(retrieved_owner, new_owner);
    }

    #[test]
    fn test_admin_management() {
        let env = soroban_sdk::Env::default();
        let owner = soroban_sdk::Address::generate(&env);
        let admin1 = soroban_sdk::Address::generate(&env);
        let admin2 = soroban_sdk::Address::generate(&env);

        ClipsNftContract::initialize(env.clone(), owner.clone(), 500);

        // Owner should be admin by default
        assert!(ClipsNftContract::is_administrator(env.clone(), owner.clone()));

        // Add admins
        ClipsNftContract::add_administrator(env.clone(), admin1.clone());
        assert!(ClipsNftContract::is_administrator(env.clone(), admin1.clone()));

        // Add another admin
        ClipsNftContract::add_administrator(env.clone(), admin2.clone());
        assert!(ClipsNftContract::is_administrator(env.clone(), admin2.clone()));

        // Remove admin
        ClipsNftContract::remove_administrator(env.clone(), admin1.clone());
        assert!(!ClipsNftContract::is_administrator(env.clone(), admin1.clone()));
    }

    #[test]
    fn test_royalty_configuration() {
        let env = soroban_sdk::Env::default();
        let owner = soroban_sdk::Address::generate(&env);

        ClipsNftContract::initialize(env.clone(), owner.clone(), 500);

        let royalty = ClipsNftContract::get_royalty_bps(env.clone());
        assert_eq!(royalty, 500);

        // Update royalty to 1000 (10%)
        ClipsNftContract::set_royalty_bps(env.clone(), 1000);
        let royalty = ClipsNftContract::get_royalty_bps(env.clone());
        assert_eq!(royalty, 1000);
    }

    #[test]
    #[should_panic]
    fn test_invalid_bps() {
        let env = soroban_sdk::Env::default();
        let owner = soroban_sdk::Address::generate(&env);

        // Should panic with BPS > 10000
        ClipsNftContract::initialize(env.clone(), owner.clone(), 10001);
    }
}
