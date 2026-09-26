//! Metadata URI validator (Issues #561, #1099).
//!
//! Validates that a metadata URI uses one of the supported protocols:
//! - `ipfs://`
//! - `https://`
//! - `ar://`  (Arweave)
//!
//! Rejects empty strings, URIs with an unsupported protocol, URIs with
//! nothing after the protocol prefix, URIs containing whitespace or control
//! characters, and URIs longer than [`MAX_METADATA_URI_LEN`] bytes.
//! [`validate_optional_metadata_uri`] accepts an empty URI for fields where
//! a URI is not required.

use soroban_sdk::String;

use crate::types::Error;

/// Maximum metadata URI length in bytes (matches the storage limit).
pub const MAX_METADATA_URI_LEN: u32 = 512;

/// Supported URI protocol prefixes.
const SUPPORTED_PREFIXES: [&[u8]; 3] = [b"ipfs://", b"https://", b"ar://"];

/// Validate a required metadata URI.
///
/// Accepted prefixes: `ipfs://`, `https://`, `ar://`
/// Returns `Err(InvalidURI)` if the URI is empty, longer than
/// [`MAX_METADATA_URI_LEN`], has an unsupported protocol, has nothing after
/// the prefix, or contains whitespace or control characters.
pub fn validate_metadata_uri(uri: &String) -> Result<(), Error> {
    let len = uri.len();
    if len == 0 || len > MAX_METADATA_URI_LEN {
        return Err(Error::InvalidURI);
    }

    // Soroban String is UTF-8 encoded bytes.
    let bytes = uri.to_bytes();

    let prefix_len = match SUPPORTED_PREFIXES
        .iter()
        .find(|prefix| has_prefix(&bytes, prefix))
    {
        Some(prefix) => prefix.len() as u32,
        None => return Err(Error::InvalidURI),
    };
    if len == prefix_len {
        return Err(Error::InvalidURI);
    }

    for byte in bytes.iter() {
        if byte <= b' ' || byte == 0x7f {
            return Err(Error::InvalidURI);
        }
    }

    Ok(())
}

/// Validate a metadata URI that may be left empty.
///
/// An empty URI passes; a non-empty URI must satisfy
/// [`validate_metadata_uri`].
pub fn validate_optional_metadata_uri(uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Ok(());
    }
    validate_metadata_uri(uri)
}

/// Returns true if `data` starts with `prefix`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Bytes, Env, String};
    #[test]
    fn valid_ipfs_uri_passes() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://QmXyZ123");
        assert!(validate_metadata_uri(&uri).is_ok());
    }
    #[test]
    fn valid_https_uri_passes() {
        let env = Env::default();
        let uri = String::from_str(&env, "https://example.com/metadata.json");
        assert!(validate_metadata_uri(&uri).is_ok());
    }
    #[test]
    fn valid_arweave_uri_passes() {
        let env = Env::default();
        let uri = String::from_str(&env, "ar://abc123");
        assert!(validate_metadata_uri(&uri).is_ok());
    }
    #[test]
    fn empty_uri_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }
    #[test]
    fn invalid_http_uri_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "http://example.com");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }
    #[test]
    fn invalid_ftp_uri_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ftp://example.com");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }
    #[test]
    fn too_short_uri_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }

    #[test]
    fn prefix_only_uri_fails() {
        let env = Env::default();
        for uri in ["ipfs://", "https://", "ar://"] {
            let uri = String::from_str(&env, uri);
            assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
        }
    }

    #[test]
    fn uri_with_whitespace_fails() {
        let env = Env::default();
        let uri = String::from_str(&env, "ipfs://Qm Xy");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
        let uri = String::from_str(&env, "https://example.com/\n");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
        let uri = String::from_str(&env, "https://example.com/\t");
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }

    fn uri_of_len(env: &Env, len: u32) -> String {
        let mut bytes = Bytes::from_slice(env, b"ipfs://");
        while bytes.len() < len {
            bytes.push_back(b'a');
        }
        let mut buf = [0u8; 600];
        bytes.copy_into_slice(&mut buf[..len as usize]);
        String::from_bytes(env, &buf[..len as usize])
    }

    #[test]
    fn uri_at_max_length_passes() {
        let env = Env::default();
        let uri = uri_of_len(&env, MAX_METADATA_URI_LEN);
        assert!(validate_metadata_uri(&uri).is_ok());
    }

    #[test]
    fn uri_over_max_length_fails() {
        let env = Env::default();
        let uri = uri_of_len(&env, MAX_METADATA_URI_LEN + 1);
        assert_eq!(validate_metadata_uri(&uri), Err(Error::InvalidURI));
    }

    #[test]
    fn optional_uri_accepts_empty() {
        let env = Env::default();
        let uri = String::from_str(&env, "");
        assert!(validate_optional_metadata_uri(&uri).is_ok());
    }

    #[test]
    fn optional_uri_still_validates_non_empty() {
        let env = Env::default();
        let ok = String::from_str(&env, "ar://abc123");
        let bad = String::from_str(&env, "ftp://example.com");
        assert!(validate_optional_metadata_uri(&ok).is_ok());
        assert_eq!(validate_optional_metadata_uri(&bad), Err(Error::InvalidURI));
    }
}
