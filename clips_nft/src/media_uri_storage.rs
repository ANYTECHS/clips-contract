//! Per-token thumbnail and preview URI storage.

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Persist the thumbnail URI for `token_id`.
pub fn set_thumbnail(env: &Env, token_id: TokenId, uri: &String) {
    env.storage()
        .persistent()
        .set(&DataKey::Thumbnail(token_id), uri);
}

/// Load the thumbnail URI for `token_id`.
pub fn get_thumbnail(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Thumbnail(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Persist the preview / image URI for `token_id`.
pub fn set_preview_uri(env: &Env, token_id: TokenId, uri: &String) {
    env.storage()
        .persistent()
        .set(&DataKey::PreviewUri(token_id), uri);
}

/// Load the preview / image URI for `token_id`.
pub fn get_preview_uri(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::PreviewUri(token_id))
        .ok_or(Error::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{Env, String};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn stores_thumbnail_and_preview() {
        with_contract(|env| {
            let thumb = String::from_str(env, "ipfs://QmThumb");
            let preview = String::from_str(env, "ipfs://QmPreview");
            set_thumbnail(env, 1, &thumb);
            set_preview_uri(env, 1, &preview);
            assert_eq!(get_thumbnail(env, 1).unwrap(), thumb);
            assert_eq!(get_preview_uri(env, 1).unwrap(), preview);
        });
    }
}
