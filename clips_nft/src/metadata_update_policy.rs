//! Metadata update policy (Issue #562).
//!
//! Governs when an NFT's metadata URI may be changed:
//!
//! - **One-time update**: each token may be updated exactly once by its owner
//!   (the update flag is set after the first change).
//! - **Admin override**: the contract admin may always update any token's
//!   metadata, bypassing the one-time restriction.
//!
//! The used-update flag is stored under `DataKey::MetadataUpdated(token_id)`
//! in persistent storage.

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error};

/// Returns `true` if the token's one-time metadata update has already been used.
pub fn is_update_used(env: &Env, token_id: u32) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::MetadataUpdated(token_id))
}

/// Mark the token's one-time metadata update as consumed.
pub fn mark_update_used(env: &Env, token_id: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::MetadataUpdated(token_id), &true);
}

/// Enforce the update policy for a given `caller`.
///
/// - If `caller` is the admin, the update is always permitted.
/// - Otherwise, the token's one-time update slot must not have been used yet.
///
/// Does **not** consume the update slot — call [`mark_update_used`] after a
/// successful write.
pub fn check_update_allowed(env: &Env, token_id: u32, caller: &Address) -> Result<(), Error> {
    // Admin override: admin can always update.
    let admin: Option<Address> = env.storage().instance().get(&DataKey::Admin);
    if let Some(ref a) = admin {
        if caller == a {
            return Ok(());
        }
    }

    // Non-admin: only one update is permitted per token.
    if is_update_used(env, token_id) {
        return Err(Error::MetadataAlreadyUpdated);
    }

    Ok(())
}
