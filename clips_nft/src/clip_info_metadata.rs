use soroban_sdk::{contracttype, Env, String};

use crate::types::{DataKey, Error, TokenId};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipInfo {
    pub clip_id: u32,
    pub duration: u32,
    pub category: String,
    pub language: String,
}

pub fn set_clip_info(env: &Env, token_id: TokenId, info: &ClipInfo) {
    env.storage()
        .persistent()
        .set(&DataKey::ClipInfo(token_id), info);
}

pub fn get_clip_info(env: &Env, token_id: TokenId) -> Result<ClipInfo, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::ClipInfo(token_id))
        .ok_or(Error::TokenNotFound)
}
