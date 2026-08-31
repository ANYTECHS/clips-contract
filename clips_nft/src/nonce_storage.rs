//! Nonce storage for signature replay prevention.
//!
//! Maintains per-address nonce counters to ensure each signed transaction
//! includes a unique nonce, preventing signature replay attacks.
//!
//! # Storage
//! Key: `DataKey::Nonce(address)` → `u64` (persistent)
//!
//! # Workflow
//! 1. Before accepting a signed mint request, retrieve the current nonce: [`retrieve_nonce`]
//! 2. Verify the request includes this nonce in the signature
//! 3. Upon successful mint, increment the nonce: [`increment_nonce`]
//! 4. Failed mints do not increment the nonce (allow retry with same nonce)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error};

/// Retrieve the current nonce for an address.
///
/// Returns 0 if the address has never minted before.
///
/// # Arguments
/// * `env` - The contract environment
/// * `address` - The signer/creator address
///
/// # Returns
/// The current nonce value for this address (0-indexed)
pub fn retrieve_nonce(env: &Env, address: &Address) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, u64>(&DataKey::Nonce(address.clone()))
        .unwrap_or(0)
}

/// Save a specific nonce value for an address.
///
/// Used primarily for testing or recovery scenarios. Use [`increment_nonce`]
/// for normal minting workflow.
///
/// # Arguments
/// * `env` - The contract environment
/// * `address` - The signer/creator address
/// * `nonce` - The nonce value to persist
pub fn save_nonce(env: &Env, address: &Address, nonce: u64) -> Result<(), Error> {
    env.storage()
        .persistent()
        .set(&DataKey::Nonce(address.clone()), &nonce);
    Ok(())
}

/// Atomically increment the nonce for an address.
///
/// Fetches the current nonce, increments it by 1, and persists the new value.
/// This ensures the next signed request must include the incremented nonce.
///
/// # Arguments
/// * `env` - The contract environment
/// * `address` - The signer/creator address
///
/// # Returns
/// The new (incremented) nonce value
///
/// # Panic
/// Panics if nonce would overflow u64 (practically impossible after 2^64 mints)
pub fn increment_nonce(env: &Env, address: &Address) -> Result<u64, Error> {
    let current = retrieve_nonce(env, address);
    let next = current.checked_add(1).expect("nonce overflow");
    save_nonce(env, address, next)?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn retrieve_nonce_returns_zero_for_new_address() {
        with_contract(|env| {
            let address = Address::generate(env);
            let nonce = retrieve_nonce(env, &address);
            assert_eq!(nonce, 0);
        });
    }

    #[test]
    fn save_nonce_persists_value() {
        with_contract(|env| {
            let address = Address::generate(env);
            let test_nonce = 42u64;

            save_nonce(env, &address, test_nonce).unwrap();
            let retrieved = retrieve_nonce(env, &address);

            assert_eq!(retrieved, test_nonce);
        });
    }

    #[test]
    fn increment_nonce_starts_from_zero() {
        with_contract(|env| {
            let address = Address::generate(env);

            let new_nonce = increment_nonce(env, &address).unwrap();
            assert_eq!(new_nonce, 1);

            let retrieved = retrieve_nonce(env, &address);
            assert_eq!(retrieved, 1);
        });
    }

    #[test]
    fn increment_nonce_increments_from_existing_value() {
        with_contract(|env| {
            let address = Address::generate(env);
            let initial = 100u64;

            save_nonce(env, &address, initial).unwrap();
            let new_nonce = increment_nonce(env, &address).unwrap();

            assert_eq!(new_nonce, initial + 1);
            let retrieved = retrieve_nonce(env, &address);
            assert_eq!(retrieved, initial + 1);
        });
    }

    #[test]
    fn multiple_increments_work_sequentially() {
        with_contract(|env| {
            let address = Address::generate(env);

            // Simulate 5 sequential mints
            for expected_nonce in 1..=5u64 {
                let actual_nonce = increment_nonce(env, &address).unwrap();
                assert_eq!(actual_nonce, expected_nonce);
            }

            let final_nonce = retrieve_nonce(env, &address);
            assert_eq!(final_nonce, 5);
        });
    }

    #[test]
    fn different_addresses_have_independent_nonces() {
        with_contract(|env| {
            let addr1 = Address::generate(env);
            let addr2 = Address::generate(env);

            save_nonce(env, &addr1, 10).unwrap();
            save_nonce(env, &addr2, 20).unwrap();

            assert_eq!(retrieve_nonce(env, &addr1), 10);
            assert_eq!(retrieve_nonce(env, &addr2), 20);

            increment_nonce(env, &addr1).unwrap();
            assert_eq!(retrieve_nonce(env, &addr1), 11);
            assert_eq!(retrieve_nonce(env, &addr2), 20);
        });
    }

    #[test]
    fn nonce_persists_across_multiple_calls() {
        with_contract(|env| {
            let address = Address::generate(env);

            save_nonce(env, &address, 5).unwrap();
            let n1 = retrieve_nonce(env, &address);
            let n2 = retrieve_nonce(env, &address);

            assert_eq!(n1, 5);
            assert_eq!(n2, 5); // Should remain unchanged
        });
    }

    #[test]
    fn save_nonce_overwrites_previous_value() {
        with_contract(|env| {
            let address = Address::generate(env);

            save_nonce(env, &address, 10).unwrap();
            assert_eq!(retrieve_nonce(env, &address), 10);

            save_nonce(env, &address, 50).unwrap();
            assert_eq!(retrieve_nonce(env, &address), 50);
        });
    }
}
