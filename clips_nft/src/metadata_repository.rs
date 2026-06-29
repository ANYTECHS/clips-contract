//! Metadata repository — encapsulates all metadata storage for NFT tokens.
//!
//! Resolves issue #437: single module for saving, loading, and updating
//! metadata URIs. All contract code that reads or writes metadata should
//! go through this module.

use soroban_sdk::{Env, String};

use crate::types::{DataKey, Error, TokenId};

/// Persist `uri` as the metadata for `token_id`.
pub fn save(env: &Env, token_id: TokenId, uri: &String) {
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
}

/// Load the metadata URI for `token_id`.
///
/// Returns `Err(TokenNotFound)` if no metadata has been stored for this token.
pub fn load(env: &Env, token_id: TokenId) -> Result<String, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Metadata(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Update the metadata URI for `token_id`.
///
/// Returns `Err(TokenNotFound)` if no metadata exists for this token.
pub fn update(env: &Env, token_id: TokenId, uri: &String) -> Result<(), Error> {
    if !env.storage().persistent().has(&DataKey::Metadata(token_id)) {
        return Err(Error::TokenNotFound);
    }
    env.storage()
        .persistent()
        .set(&DataKey::Metadata(token_id), uri);
    Ok(())
}

/// Remove metadata for `token_id`. Used during token burn.
pub fn remove(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Metadata(token_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn save_and_load() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTest");
        save(&env, 1, &uri);
        assert_eq!(load(&env, 1).unwrap(), uri);
    }

    #[test]
    fn load_missing_returns_not_found() {
        let env = Env::default();
        assert_eq!(load(&env, 99), Err(Error::TokenNotFound));
    }

    #[test]
    fn update_existing() {
        let env = Env::default();
        let uri1 = String::from_str(&env, "ipfs://QmOld");
        let uri2 = String::from_str(&env, "ipfs://QmNew");
        save(&env, 1, &uri1);
        update(&env, 1, &uri2).unwrap();
        assert_eq!(load(&env, 1).unwrap(), uri2);
    }

    #[test]
    fn update_missing_returns_not_found() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTest");
        assert_eq!(update(&env, 99, &uri), Err(Error::TokenNotFound));
    }

    #[test]
    fn remove_clears_metadata() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTest");
        save(&env, 1, &uri);
        remove(&env, 1);
        assert_eq!(load(&env, 1), Err(Error::TokenNotFound));
    }
}
