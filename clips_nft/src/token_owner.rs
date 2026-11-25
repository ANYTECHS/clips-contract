//! Token owner storage.
//!
//! Stores ownership information for every NFT.
//!
//! # Storage
//! Key: `DataKey::TokenOwner(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, TokenId};

/// Save the owner of `token_id`. Overwrites any existing value (also used for updates).
pub fn set_token_owner(env: &Env, token_id: TokenId, owner: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::TokenOwner(token_id), owner);
}

/// Return the owner of `token_id`, or `None` if not set.
pub fn get_token_owner(env: &Env, token_id: TokenId) -> Option<Address> {
    env.storage().persistent().get(&DataKey::TokenOwner(token_id))
}
