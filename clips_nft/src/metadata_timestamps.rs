use soroban_sdk::{contracttype, Env};

use crate::types::{DataKey, Error, TokenId};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataTimestamps {
    pub created_at: u64,
    pub updated_at: u64,
}

pub fn set_timestamps(env: &Env, token_id: TokenId, timestamps: &MetadataTimestamps) {
    env.storage()
        .persistent()
        .set(&DataKey::MetadataTimestamps(token_id), timestamps);
}

pub fn get_timestamps(env: &Env, token_id: TokenId) -> Result<MetadataTimestamps, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::MetadataTimestamps(token_id))
        .ok_or(Error::TokenNotFound)
}

pub fn touch_updated_at(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let mut ts = get_timestamps(env, token_id)?;
    ts.updated_at = env.ledger().timestamp();
    set_timestamps(env, token_id, &ts);
    Ok(())
}
