//! Social platform metadata — store and retrieve the originating platform for a clip.

use soroban_sdk::Env;

use crate::types::{DataKey, Error, TokenId};

/// Supported social platforms.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SocialPlatform {
    TikTok = 0,
    YouTube = 1,
    Instagram = 2,
    X = 3,
    Facebook = 4,
}

impl SocialPlatform {
    fn from_u32(v: u32) -> Result<Self, Error> {
        match v {
            0 => Ok(Self::TikTok),
            1 => Ok(Self::YouTube),
            2 => Ok(Self::Instagram),
            3 => Ok(Self::X),
            4 => Ok(Self::Facebook),
            _ => Err(Error::InvalidBasisPoints),
        }
    }
}

/// Persist the social platform for `token_id`.
pub fn set_platform(env: &Env, token_id: TokenId, platform: SocialPlatform) {
    env.storage()
        .persistent()
        .set(&DataKey::SocialPlatform(token_id), &(platform as u32));
}

/// Return the social platform for `token_id`, or `Err(TokenNotFound)` if not set.
pub fn get_platform(env: &Env, token_id: TokenId) -> Result<SocialPlatform, Error> {
    let v: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::SocialPlatform(token_id))
        .ok_or(Error::TokenNotFound)?;
    SocialPlatform::from_u32(v)
}
