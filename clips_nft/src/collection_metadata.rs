use soroban_sdk::{contracttype, Env, String};

use crate::types::{DataKey, Error};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionMetadata {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub banner_uri: String,
}

pub fn set_collection_metadata(env: &Env, meta: &CollectionMetadata) {
    env.storage().instance().set(&DataKey::CollectionMetadata, meta);
}

pub fn get_collection_metadata(env: &Env) -> Result<CollectionMetadata, Error> {
    env.storage()
        .instance()
        .get(&DataKey::CollectionMetadata)
        .ok_or(Error::NotInitialized)
}
