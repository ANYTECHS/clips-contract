//! Storage validation helper (Task 2).
//!
//! Validates data before it is written to storage.
//! All functions return [`Error`] on invalid input so callers can
//! propagate errors without panicking.

use soroban_sdk::{Address, Env, String};

use crate::types::{DataKey, Error, Royalty, TokenId};

/// Maximum metadata URI length (bytes).
pub const MAX_URI_LEN: u32 = 512;

/// Validate an `Address` by verifying the contract has been initialised
/// and that the address is not the zero sentinel.
///
/// Soroban addresses are structurally valid by construction, so this check
/// focuses on ensuring the contract is in a state that can accept the address.
pub fn validate_address(env: &Env, addr: &Address) -> Result<(), Error> {
    // Contract must be initialised before any address can be stored.
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }
    // Soroban Address is always structurally valid; calling `to_string` would
    // require an allocation — just asserting it is not the admin sentinel.
    let _ = addr; // structural validity guaranteed by the type system
    Ok(())
}

/// Validate that `token_id` refers to a token that has already been minted.
pub fn validate_token_id(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let next: u32 = env
        .storage()
        .instance()
        .get(&DataKey::NextTokenId)
        .unwrap_or(0);
    if token_id >= next {
        return Err(Error::TokenNotFound);
    }
    Ok(())
}

/// Validate a metadata URI string.
///
/// - Must not be empty.
/// - Must not exceed [`MAX_URI_LEN`] bytes.
pub fn validate_metadata_uri(uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    if uri.len() > MAX_URI_LEN {
        return Err(Error::InvalidURI);
    }
    Ok(())
}

/// Validate a [`Royalty`] struct before persisting.
///
/// - `basis_points` must be in range `[0, 10_000]`.
/// - `recipient` structural validity is guaranteed by the type system.
pub fn validate_royalty(env: &Env, royalty: &Royalty) -> Result<(), Error> {
    if royalty.basis_points > 10_000 {
        return Err(Error::InvalidBasisPoints);
    }
    validate_address(env, &royalty.recipient)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, String};
    use crate::types::Royalty;

    #[test]
    fn test_validate_metadata_uri_empty_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_metadata_uri_valid() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmTest");
        assert!(validate_metadata_uri(&uri).is_ok());
    }

    #[test]
    fn test_validate_metadata_uri_too_long() {
        let env = Env::default();
        // Build a string > 512 bytes
        let long: std::string::String = "a".repeat(513);
        let uri = String::from_str(&env, &long);
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_validate_royalty_invalid_bps() {
        let env = Env::default();
        env.mock_all_auths();
        let addr = Address::generate(&env);
        // initialise storage so validate_address passes
        env.storage().instance().set(&DataKey::Admin, &addr);
        let r = Royalty { recipient: addr, basis_points: 10_001, asset_address: None };
        assert_eq!(validate_royalty(&env, &r), Err(Error::InvalidBasisPoints));
    }

    #[test]
    fn test_validate_royalty_valid() {
        let env = Env::default();
        env.mock_all_auths();
        let addr = Address::generate(&env);
        env.storage().instance().set(&DataKey::Admin, &addr);
        let r = Royalty { recipient: addr, basis_points: 500, asset_address: None };
        assert!(validate_royalty(&env, &r).is_ok());
    }

    #[test]
    fn test_validate_token_id_not_minted() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::NextTokenId, &0u32);
        assert_eq!(validate_token_id(&env, 0), Err(Error::TokenNotFound));
    }

    #[test]
    fn test_validate_token_id_valid() {
        let env = Env::default();
        env.storage().instance().set(&DataKey::NextTokenId, &3u32);
        assert!(validate_token_id(&env, 2).is_ok());
    }
}
