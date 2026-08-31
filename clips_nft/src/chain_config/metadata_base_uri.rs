//! Metadata Base URI Configuration — resolves issue #476.
//!
//! Allows administrators to configure the default base URI that is prepended
//! to token IDs when building individual token metadata URLs.
//!
//! # Storage
//! Key: [`ConfigKey::MetadataBaseUri`] (instance storage)
//!
//! # Validation
//! The base URI must be non-empty and begin with one of the supported
//! protocols: `https://`, `http://`, `ipfs://`, or `ar://` (Arweave).
//!
//! # Example
//! ```text
//! set_metadata_base_uri(env, String::from_str(env, "https://api.example.com/metadata/"))
//! // → Ok(())
//!
//! get_metadata_base_uri(env)
//! // → Some("https://api.example.com/metadata/")
//! ```

use soroban_sdk::{Env, String};

use crate::types::Error;

use super::keys::ConfigKey;

// ─── Validation ──────────────────────────────────────────────────────────────

/// Validate a metadata base URI.
///
/// Rules:
/// - Must be non-empty.
/// - Must start with `https://`, `http://`, `ipfs://`, or `ar://`.
///
/// Returns [`Error::InvalidURI`] on failure.
pub fn validate_metadata_base_uri(uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    let bytes = uri.to_bytes();

    if has_prefix(&bytes, b"https://")
        || has_prefix(&bytes, b"http://")
        || has_prefix(&bytes, b"ipfs://")
        || has_prefix(&bytes, b"ar://")
    {
        return Ok(());
    }

    Err(Error::InvalidURI)
}

// ─── Getter ───────────────────────────────────────────────────────────────────

/// Return the stored metadata base URI.
///
/// Returns `None` if no base URI has been configured yet.
pub fn get_metadata_base_uri(env: &Env) -> Option<String> {
    env.storage().instance().get(&ConfigKey::MetadataBaseUri)
}

// ─── Setter ───────────────────────────────────────────────────────────────────

/// Persist the metadata base URI after validation.
///
/// # Errors
/// Returns [`Error::InvalidURI`] if `uri` is empty or uses an unsupported
/// scheme.
pub fn set_metadata_base_uri(env: &Env, uri: String) -> Result<(), Error> {
    validate_metadata_base_uri(&uri)?;
    env.storage()
        .instance()
        .set(&ConfigKey::MetadataBaseUri, &uri);
    Ok(())
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

fn has_prefix(data: &soroban_sdk::Bytes, prefix: &[u8]) -> bool {
    if data.len() < prefix.len() as u32 {
        return false;
    }
    for (i, &expected) in prefix.iter().enumerate() {
        if data.get(i as u32) != Some(expected) {
            return false;
        }
    }
    true
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use soroban_sdk::{Env, String};

    use super::*;

    fn new_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_set_and_get_https_base_uri() {
        let env = new_env();
        let uri = String::from_str(&env, "https://api.example.com/metadata/");
        set_metadata_base_uri(&env, uri.clone()).expect("should set URI");
        let stored = get_metadata_base_uri(&env).expect("should have URI");
        assert_eq!(stored, uri);
    }

    #[test]
    fn test_set_ipfs_base_uri() {
        let env = new_env();
        let uri = String::from_str(&env, "ipfs://QmBaseHash/");
        assert!(set_metadata_base_uri(&env, uri).is_ok());
    }

    #[test]
    fn test_set_arweave_base_uri() {
        let env = new_env();
        let uri = String::from_str(&env, "ar://SomeArweaveTx/");
        assert!(set_metadata_base_uri(&env, uri).is_ok());
    }

    #[test]
    fn test_set_http_base_uri() {
        let env = new_env();
        let uri = String::from_str(&env, "http://localhost:3000/metadata/");
        assert!(set_metadata_base_uri(&env, uri).is_ok());
    }

    #[test]
    fn test_get_returns_none_when_not_set() {
        let env = new_env();
        assert!(get_metadata_base_uri(&env).is_none());
    }

    #[test]
    fn test_empty_uri_rejected() {
        let env = new_env();
        let uri = String::from_str(&env, "");
        assert_eq!(set_metadata_base_uri(&env, uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_unsupported_scheme_rejected() {
        let env = new_env();
        let uri = String::from_str(&env, "ftp://files.example.com/metadata/");
        assert_eq!(set_metadata_base_uri(&env, uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_arbitrary_string_rejected() {
        let env = new_env();
        let uri = String::from_str(&env, "not-a-uri");
        assert_eq!(set_metadata_base_uri(&env, uri), Err(Error::InvalidURI));
    }

    #[test]
    fn test_overwrite_base_uri() {
        let env = new_env();
        let first = String::from_str(&env, "https://old.example.com/metadata/");
        let second = String::from_str(&env, "https://new.example.com/metadata/");
        set_metadata_base_uri(&env, first).unwrap();
        set_metadata_base_uri(&env, second.clone()).unwrap();
        let stored = get_metadata_base_uri(&env).unwrap();
        assert_eq!(stored, second);
    }
}
