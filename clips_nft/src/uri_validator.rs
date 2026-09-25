//! URI validator (issue #1099).
//!
//! Validates the URIs an NFT carries — the metadata URI and the optional image,
//! animation and external URLs.
//!
//! The rules already exist, but they are spread over five modules that overlap:
//! `metadata::validation` (scheme + per-field rules), `metadata_size` (byte cap),
//! `storage_validator`, `metadata_uri_validator`, and
//! `chain_config::metadata_base_uri`. A caller that wants "is this URI
//! acceptable" has to know which of them to combine, and the answer differs
//! depending on which one it picks. This module is the single entry point:
//!
//! 1. **Present** — an empty string is [`Error::InvalidURI`].
//! 2. **Within the cap** — over `metadata_size::MAX_METADATA_URI_BYTES` bytes is
//!    [`Error::MetadataSizeTooLarge`], so "too big" is distinguishable from
//!    "malformed".
//! 3. **Supported scheme** — outside `metadata::validation::SUPPORTED_PROTOCOLS`
//!    is [`Error::UnsupportedProtocol`].
//!
//! ```ignore
//! uri_validator::validate_uri(&env, &metadata_uri)?;
//! uri_validator::validate_optional_uri(&env, &image)?;
//! ```
//!
//! It deliberately does not reimplement any of the three checks: each one stays
//! defined in the module that owns it, which is also why the scheme check in
//! `metadata::validation::validate_url` had to be fixed for this validator to
//! pass anything at all (see that function).

use soroban_sdk::{Env, String};

use crate::metadata::validation::validate_url;
use crate::metadata_size::validate_metadata_size;
use crate::types::Error;

/// Validate a required URI.
///
/// # Errors
/// * [`Error::InvalidURI`] — the URI is empty.
/// * [`Error::MetadataSizeTooLarge`] — it exceeds the documented byte cap.
/// * [`Error::UnsupportedProtocol`] — its scheme is not supported.
pub fn validate_uri(env: &Env, uri: &String) -> Result<(), Error> {
    if uri.is_empty() {
        return Err(Error::InvalidURI);
    }

    validate_metadata_size(uri)?;
    validate_url(env, uri)
}

/// Validate an optional URI: absent and empty values are accepted, anything
/// present is checked with [`validate_uri`].
///
/// # Errors
/// * the errors of [`validate_uri`], for a value that is present and non-empty.
pub fn validate_optional_uri(env: &Env, uri: &Option<String>) -> Result<(), Error> {
    match uri {
        Some(value) if !value.is_empty() => validate_uri(env, value),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_size::MAX_METADATA_URI_BYTES;
    use soroban_sdk::{Env, String};

    fn s(env: &Env, value: &str) -> String {
        String::from_str(env, value)
    }

    #[test]
    fn empty_uri_is_rejected() {
        let env = Env::default();
        assert_eq!(validate_uri(&env, &s(&env, "")), Err(Error::InvalidURI));
    }

    #[test]
    fn uri_over_the_cap_is_a_size_error_not_a_format_one() {
        let env = Env::default();
        let long = "i".repeat((MAX_METADATA_URI_BYTES + 1) as usize);

        assert_eq!(
            validate_uri(&env, &s(&env, &long)),
            Err(Error::MetadataSizeTooLarge)
        );
    }

    #[test]
    fn uri_at_the_cap_is_accepted() {
        let env = Env::default();
        // "ipfs://" + filler, exactly at the limit.
        let prefix = "ipfs://";
        let filler = "a".repeat(MAX_METADATA_URI_BYTES as usize - prefix.len());

        assert_eq!(
            validate_uri(&env, &s(&env, &alloc::format!("{prefix}{filler}"))),
            Ok(())
        );
    }

    #[test]
    fn unsupported_scheme_is_rejected() {
        let env = Env::default();

        // The three supported schemes are in SUPPORTED_PROTOCOLS; anything else
        // is refused, including a scheme that merely looks similar.
        assert_eq!(
            validate_uri(&env, &s(&env, "ftp://example.com/clip.json")),
            Err(Error::UnsupportedProtocol)
        );
        assert_eq!(
            validate_uri(&env, &s(&env, "http://example.com/clip.json")),
            Err(Error::UnsupportedProtocol),
            "http is not https"
        );
    }

    #[test]
    fn supported_schemes_are_accepted() {
        let env = Env::default();

        for uri in [
            "ipfs://QmTestUri",
            "https://example.com/clip.json",
            "ar://TestArweaveId",
        ] {
            assert_eq!(validate_uri(&env, &s(&env, uri)), Ok(()), "{uri}");
        }
    }

    #[test]
    fn empty_optional_uri_is_accepted() {
        let env = Env::default();

        assert_eq!(validate_optional_uri(&env, &None), Ok(()));
        assert_eq!(validate_optional_uri(&env, &Some(s(&env, ""))), Ok(()));
    }

    #[test]
    fn present_optional_uri_is_validated() {
        let env = Env::default();
        let long = "i".repeat((MAX_METADATA_URI_BYTES + 1) as usize);

        assert_eq!(
            validate_optional_uri(&env, &Some(s(&env, "ipfs://QmTestUri"))),
            Ok(())
        );
        assert_eq!(
            validate_optional_uri(&env, &Some(s(&env, "ftp://example.com/a.png"))),
            Err(Error::UnsupportedProtocol)
        );
        assert_eq!(
            validate_optional_uri(&env, &Some(s(&env, &long))),
            Err(Error::MetadataSizeTooLarge)
        );
    }
}
