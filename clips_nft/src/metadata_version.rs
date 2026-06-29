use soroban_sdk::{contracttype, Env};

use crate::types::DataKey;

pub const DEFAULT_METADATA_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataVersion {
    pub version: u32,
}

impl MetadataVersion {
    pub fn default_version() -> Self {
        Self { version: DEFAULT_METADATA_VERSION }
    }
}

pub fn get_metadata_version(env: &Env) -> MetadataVersion {
    env.storage()
        .instance()
        .get(&DataKey::MetadataVersion)
        .unwrap_or_else(MetadataVersion::default_version)
}

pub fn set_metadata_version(env: &Env, v: MetadataVersion) {
    env.storage().instance().set(&DataKey::MetadataVersion, &v);
}
