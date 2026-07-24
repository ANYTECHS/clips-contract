//! IPFS Gateway Configuration — resolves issue #477.
//!
//! Stores and validates the default IPFS gateway URL used by the contract
//! when constructing token URIs or resolving IPFS-hosted metadata.
//!
//! # Storage
//! Key: [`ConfigKey::IpfsGateway`] (instance storage)
//!
//! # Validation
//! The gateway URL must be non-empty and begin with either `https://` or
//! `http://`.  Plain `ipfs://` scheme is intentionally rejected here because
//! a *gateway* URL must be an HTTP(S) endpoint, not a raw IPFS URI.
//!
//! # Example
//! ```text
//! set_ipfs_gateway(env, String::from_str(env, "https://ipfs.io/ipfs/"))
//! // → Ok(())
//!
//! get_ipfs_gateway(env)
//! // → Some("https://ipfs.io/ipfs/")
//! ```

use soroban_sdk::{Env, String};

use crate::types::Error;

use super::keys::ConfigKey;

/// Default public IPFS gateway used when none has been configured.
pub const DEFAULT_IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

// ─── Validation ──────────────────────────────────────────────────────────────

/// Validate an IPFS gateway URL.
///
/// Rules:
/// - Must be non-empty.
/// - Must start with `https://` or `http://`.
///
/// Returns [`Error::InvalidURI`] on failure.
pub fn validate_ipfs_gateway(url: &String) -> Result<(), Error> {
    if url.len() == 0 {
        return Err(Error::InvalidURI);
    }

    let bytes = url.to_bytes();

    if has_prefix(&bytes, b"https://") || has_prefix(&bytes, b"http://") {
        return Ok(());
    }

    Err(Error::InvalidURI)
}

// ─── Getter ───────────────────────────────────────────────────────────────────

/// Return the stored IPFS gateway URL.
///
/// Returns `None` if no gateway has been configured yet.
pub fn get_ipfs_gateway(env: &Env) -> Option<String> {
    env.storage()
        .instance()
        .get(&ConfigKey::IpfsGateway)
}

// ─── Setter ───────────────────────────────────────────────────────────────────

/// Persist the IPFS gateway URL after validation.
///
/// # Errors
/// Returns [`Error::InvalidURI`] if `url` is empty or does not start with
/// `https://` or `http://`.
pub fn set_ipfs_gateway(env: &Env, url: String) -> Result<(), Error> {
    validate_ipfs_gateway(&url)?;
    env.storage()
        .instance()
        .set(&ConfigKey::IpfsGateway, &url);
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
    use soroban_sdk::{testutils::Address as _, Env, String};

    use super::*;

    fn new_env() -> Env {
        Env::default()
    }

    #[test]
    fn test_set_and_get_ipfs_gateway() {
        let env = new_env();
        let url = String::from_str(&env, "https://ipfs.io/ipfs/");
        set_ipfs_gateway(&env, url.clone()).expect("should set gateway");
        let stored = get_ipfs_gateway(&env).expect("should have gateway");
        assert_eq!(stored, url);
    }

    #[test]
    fn test_set_http_gateway() {
        let env = new_env();
        let url = String::from_str(&env, "http://localhost:8080/ipfs/");
        assert!(set_ipfs_gateway(&env, url).is_ok());
    }

    #[test]
    fn test_get_returns_none_when_not_set() {
        let env = new_env();
        assert!(get_ipfs_gateway(&env).is_none());
    }

    #[test]
    fn test_empty_url_rejected() {
        let env = new_env();
        let url = String::from_str(&env, "");
        assert_eq!(set_ipfs_gateway(&env, url), Err(Error::InvalidURI));
    }

    #[test]
    fn test_ipfs_scheme_rejected() {
        let env = new_env();
        let url = String::from_str(&env, "ipfs://QmSomeHash");
        assert_eq!(set_ipfs_gateway(&env, url), Err(Error::InvalidURI));
    }

    #[test]
    fn test_arbitrary_string_rejected() {
        let env = new_env();
        let url = String::from_str(&env, "not-a-url");
        assert_eq!(set_ipfs_gateway(&env, url), Err(Error::InvalidURI));
    }

    #[test]
    fn test_overwrite_gateway() {
        let env = new_env();
        let first = String::from_str(&env, "https://cloudflare-ipfs.com/ipfs/");
        let second = String::from_str(&env, "https://gateway.pinata.cloud/ipfs/");
        set_ipfs_gateway(&env, first).unwrap();
        set_ipfs_gateway(&env, second.clone()).unwrap();
        let stored = get_ipfs_gateway(&env).unwrap();
        assert_eq!(stored, second);
    }
}
