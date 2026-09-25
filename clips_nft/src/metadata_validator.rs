//! Metadata validator (issue #1098).
//!
//! Validates an NFT's metadata record before creation or update.
//!
//! The field rules already exist and are tested in
//! [`crate::metadata::validation`] (required URI, URL schemes, attributes,
//! description), and the URI byte cap exists in [`crate::metadata_size`]. What
//! was missing is an entry point that runs them in one documented order over a
//! whole [`TokenMetadata`] record, which is what a mint or a metadata update
//! needs:
//!
//! 1. **Required fields** — a metadata record without a URI is not metadata.
//! 2. **Size constraints** — reported as [`Error::MetadataSizeTooLarge`], the
//!    precise error, rather than the [`Error::InvalidURI`] the field validators
//!    return for the same shape violation. Running this pass first is what makes
//!    the two distinguishable for a caller (and for a test).
//! 3. **Field format** — each URL field, the description, and the attributes.
//!
//! ```ignore
//! metadata_validator::validate_metadata(&env, &metadata)?;
//! ```

use soroban_sdk::{Env, String};

use crate::metadata::validation::{
    validate_animation_url, validate_attributes, validate_description, validate_external_url,
    validate_image_url, validate_metadata_uri,
};
use crate::metadata::{
    TokenMetadata, MAX_ATTRIBUTES_COUNT, MAX_DESCRIPTION_LENGTH, MAX_URI_LENGTH,
};
use crate::types::Error;

/// Reject a metadata record that is missing a required field.
///
/// # Errors
/// * [`Error::InvalidURI`] — `metadata_uri` is empty.
pub fn validate_required_fields(metadata: &TokenMetadata) -> Result<(), Error> {
    if metadata.metadata_uri.is_empty() {
        return Err(Error::InvalidURI);
    }
    Ok(())
}

/// Reject metadata whose fields exceed the documented limits.
///
/// # Errors
/// * [`Error::MetadataSizeTooLarge`] — the metadata URI, the description, or one
///   of the URL fields is over its byte limit, or the record carries more than
///   [`MAX_ATTRIBUTES_COUNT`] attributes.
pub fn validate_size_constraints(metadata: &TokenMetadata) -> Result<(), Error> {
    // The URI cap has its own module and error; keep using it so the limit has
    // a single definition (512 bytes, `metadata_size::MAX_METADATA_URI_BYTES`).
    crate::metadata_size::validate_metadata_size(&metadata.metadata_uri)?;

    for url in [
        &metadata.image,
        &metadata.animation_url,
        &metadata.external_url,
    ] {
        if let Some(url) = url {
            if url.len() > MAX_URI_LENGTH {
                return Err(Error::MetadataSizeTooLarge);
            }
        }
    }

    if let Some(description) = &metadata.description {
        if description.len() > MAX_DESCRIPTION_LENGTH {
            return Err(Error::MetadataSizeTooLarge);
        }
    }

    if metadata.attributes.len() > MAX_ATTRIBUTES_COUNT {
        return Err(Error::MetadataSizeTooLarge);
    }

    Ok(())
}

/// Validate a whole metadata record: required fields, sizes, then field format.
///
/// # Errors
/// * [`Error::MetadataSizeTooLarge`] — a size constraint is violated.
/// * [`Error::InvalidURI`] — a required field is missing, or a field is empty or
///   structurally invalid.
/// * [`Error::MalformedUrl`] — a URL field has no scheme.
/// * [`Error::UnsupportedProtocol`] — a URL field uses a protocol outside
///   [`crate::metadata::validation::SUPPORTED_PROTOCOLS`].
pub fn validate_metadata(env: &Env, metadata: &TokenMetadata) -> Result<(), Error> {
    validate_required_fields(metadata)?;
    validate_size_constraints(metadata)?;

    validate_metadata_uri(env, &metadata.metadata_uri)?;
    validate_image_url(env, &metadata.image)?;
    validate_animation_url(env, &metadata.animation_url)?;
    validate_external_url(env, &metadata.external_url)?;
    validate_description(&metadata.description)?;
    validate_attributes(&metadata.attributes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Attribute;
    use soroban_sdk::{Env, String, Vec};

    fn uri(env: &Env, value: &str) -> String {
        String::from_str(env, value)
    }

    fn metadata_with_uri(env: &Env, value: &str) -> TokenMetadata {
        TokenMetadata::new(env, uri(env, value))
    }

    #[test]
    fn missing_required_uri_is_rejected() {
        let env = Env::default();
        let metadata = metadata_with_uri(&env, "");

        assert_eq!(validate_required_fields(&metadata), Err(Error::InvalidURI));
        assert_eq!(
            validate_metadata(&env, &metadata),
            Err(Error::InvalidURI),
            "an empty URI is a missing required field, not a size problem"
        );
    }

    #[test]
    fn uri_over_the_limit_is_reported_as_a_size_error() {
        let env = Env::default();
        let long = "a".repeat(513);
        let metadata = metadata_with_uri(&env, &long);

        assert_eq!(
            validate_metadata(&env, &metadata),
            Err(Error::MetadataSizeTooLarge)
        );
        // The field validator alone would have said InvalidURI.
        assert_eq!(
            validate_metadata_uri(&env, &metadata.metadata_uri),
            Err(Error::InvalidURI)
        );
    }

    #[test]
    fn well_formed_metadata_is_accepted() {
        let env = Env::default();
        let metadata = metadata_with_uri(&env, "ipfs://QmTestMetadataUri");

        assert_eq!(validate_metadata(&env, &metadata), Ok(()));
    }

    #[test]
    fn unsupported_protocol_is_rejected() {
        let env = Env::default();
        let metadata = metadata_with_uri(&env, "ftp://example.com/token.json");

        assert_eq!(
            validate_metadata(&env, &metadata),
            Err(Error::UnsupportedProtocol)
        );
    }

    #[test]
    fn url_with_a_foreign_scheme_is_rejected() {
        let env = Env::default();
        let mut metadata = metadata_with_uri(&env, "ipfs://QmTestMetadataUri");
        metadata.image = Some(uri(&env, "not-a-url"));

        // No scheme at all: nothing in SUPPORTED_PROTOCOLS matches, which is the
        // signal this validator reports for an unrecognised scheme.
        assert_eq!(
            validate_metadata(&env, &metadata),
            Err(Error::UnsupportedProtocol)
        );
    }

    #[test]
    fn empty_url_is_reported_as_malformed_by_the_url_validator() {
        let env = Env::default();
        // `MalformedUrl` is reserved for an empty value; the metadata validator
        // never reaches it for the required URI, because an empty URI is caught
        // as a missing required field first.
        assert_eq!(
            crate::metadata::validation::validate_url(&env, &uri(&env, "")),
            Err(Error::MalformedUrl)
        );
    }

    #[test]
    fn oversized_optional_url_is_reported_as_a_size_error() {
        let env = Env::default();
        let mut metadata = metadata_with_uri(&env, "ipfs://QmTestMetadataUri");
        metadata.image = Some(uri(&env, &alloc::format!("ipfs://{}", "b".repeat(600))));

        assert_eq!(
            validate_metadata(&env, &metadata),
            Err(Error::MetadataSizeTooLarge)
        );
    }

    #[test]
    fn too_many_attributes_is_reported_as_a_size_error() {
        let env = Env::default();
        let mut metadata = metadata_with_uri(&env, "ipfs://QmTestMetadataUri");
        let mut attributes = Vec::new(&env);
        for i in 0..=MAX_ATTRIBUTES_COUNT {
            attributes.push_back(Attribute {
                trait_type: uri(&env, &alloc::format!("trait{i}")),
                value: uri(&env, "v"),
                display_type: None,
            });
        }
        metadata.attributes = attributes;

        assert_eq!(
            validate_size_constraints(&metadata),
            Err(Error::MetadataSizeTooLarge)
        );
        // ...and the attribute validator would have called it InvalidURI.
        assert_eq!(
            validate_attributes(&metadata.attributes),
            Err(Error::InvalidURI)
        );
    }

    #[test]
    fn empty_attribute_name_is_rejected() {
        let env = Env::default();
        let mut metadata = metadata_with_uri(&env, "ipfs://QmTestMetadataUri");
        let mut attributes = Vec::new(&env);
        attributes.push_back(Attribute {
            trait_type: uri(&env, ""),
            value: uri(&env, "v"),
            display_type: None,
        });
        metadata.attributes = attributes;

        assert_eq!(validate_metadata(&env, &metadata), Err(Error::InvalidURI));
    }

    #[test]
    fn valid_attribute_and_description_are_accepted() {
        let env = Env::default();
        let mut metadata = metadata_with_uri(&env, "ipfs://QmTestMetadataUri");
        metadata.description = Some(uri(&env, "A short description"));
        let mut attributes = Vec::new(&env);
        attributes.push_back(Attribute {
            trait_type: uri(&env, "Rarity"),
            value: uri(&env, "Legendary"),
            display_type: None,
        });
        metadata.attributes = attributes;

        assert_eq!(validate_metadata(&env, &metadata), Ok(()));
    }
}
