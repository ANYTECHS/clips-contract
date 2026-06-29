//! Configuration update guard (Issue #485).
//!
//! Restricts configuration updates to authorized administrators.
//! Provides a reusable guard that validates caller identity before
//! allowing any configuration mutation.

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error};

/// Validate that the caller is the contract owner/admin and require auth.
///
/// This is the single entry point for all configuration update authorization.
/// It checks:
/// 1. The contract has been initialized (admin exists).
/// 2. The caller matches the stored admin address.
/// 3. The caller has signed the invocation (`require_auth`).
///
/// # Errors
/// - [`Error::NotInitialized`] if the contract has not been initialized.
/// - [`Error::UnauthorizedConfigurationUpdate`] if the caller is not admin.
pub fn require_config_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    if *caller != admin {
        return Err(Error::UnauthorizedConfigurationUpdate);
    }
    caller.require_auth();
    Ok(())
}
