//! NFT collection storage.
//!
//! Stores NFT collection information including name and symbol.
//!
//! # Storage
//! Key: `DataKey::CollectionInfo` (instance storage)

use soroban_sdk::{contracttype, Env, String};

use crate::types::DataKey;

/// Collection metadata: display name and symbol.
#[contracttype]
#[derive(Clone)]
pub struct CollectionInfo {
    pub name: String,
    pub symbol: String,
}

/// Save collection info. Overwrites any existing value (also used for updates).
pub fn set_collection_info(env: &Env, info: &CollectionInfo) {
    env.storage().instance().set(&DataKey::CollectionInfo, info);
}

/// Return the stored collection info, or `None` if not set.
pub fn get_collection_info(env: &Env) -> Option<CollectionInfo> {
    env.storage().instance().get(&DataKey::CollectionInfo)
}
