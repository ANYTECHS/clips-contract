//! Metadata validator.
//!
//! Validates NFT metadata before it is accepted for creation or update.
//! This keeps the contract from persisting malformed or oversized metadata and
//! ensures optional metadata fields remain structurally valid when present.

use soroban_sdk::{Env, String, Vec};

use crate::metadata::{
    validate_animation_url, validate_attributes, validate_description, validate_external_url,
    validate_image_url, validate_metadata_uri, Attribute, ClipMetadata,
};
use crate::metadata_config::validate_metadata_size;
use crate::types::Error;

/// Validate a complete metadata payload before creation or updates.
pub fn validate_metadata(env: &Env, metadata: &ClipMetadata) -> Result<(), Error> {
    if metadata.metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    validate_metadata_uri(env, &metadata.metadata_uri)?;
    validate_metadata_size(env, &metadata.metadata_uri)?;

    if metadata.image.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::InvalidImage);
    }
    if metadata.animation_url.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::InvalidURI);
    }
    if metadata.external_url.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::InvalidURI);
    }
    if metadata.description.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::EmptyDescription);
    }

    validate_image_url(env, &metadata.image)?;
    validate_animation_url(env, &metadata.animation_url)?;
    validate_external_url(env, &metadata.external_url)?;
    validate_description(&metadata.description)?;
    validate_attributes(&metadata.attributes)?;

    Ok(())
}

/// Validate a metadata URI and optional field bundle without requiring a full
/// `ClipMetadata` struct.
pub fn validate_metadata_fields(
    env: &Env,
    metadata_uri: &String,
    image: &Option<String>,
    animation_url: &Option<String>,
    description: &Option<String>,
    external_url: &Option<String>,
    attributes: &Vec<Attribute>,
) -> Result<(), Error> {
    if metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    validate_metadata_uri(env, metadata_uri)?;
    validate_metadata_size(env, metadata_uri)?;

    if image.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::InvalidImage);
    }
    if animation_url.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::InvalidURI);
    }
    if external_url.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::InvalidURI);
    }
    if description.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(Error::EmptyDescription);
    }

    validate_image_url(env, image)?;
    validate_animation_url(env, animation_url)?;
    validate_external_url(env, external_url)?;
    validate_description(description)?;
    validate_attributes(attributes)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Attribute;
    use crate::metadata_config::set_max_metadata_size;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env, String, Vec};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn valid_metadata(env: &Env) -> ClipMetadata {
        let uri = String::from_str(env, "ipfs://QmValidMetadata");
        ClipMetadata {
            clip_id: 1,
            metadata_uri: uri,
            image: Some(String::from_str(env, "https://example.com/image.png")),
            thumbnail: None,
            animation_url: Some(String::from_str(env, "ipfs://QmVideoHash")),
            description: Some(String::from_str(env, "valid description")),
            external_url: Some(String::from_str(env, "https://example.com/clip")),
            attributes: Vec::new(env),
        }
    }

    #[test]
    fn accepts_valid_metadata() {
        with_contract(|env| {
            let metadata = valid_metadata(env);
            assert!(validate_metadata(env, &metadata).is_ok());
        });
    }

    #[test]
    fn rejects_empty_metadata_uri() {
        with_contract(|env| {
            let mut metadata = valid_metadata(env);
            metadata.metadata_uri = String::from_str(env, "");
            assert_eq!(validate_metadata(env, &metadata), Err(Error::InvalidURI));
        });
    }

    #[test]
    fn rejects_invalid_protocol() {
        with_contract(|env| {
            let mut metadata = valid_metadata(env);
            metadata.metadata_uri = String::from_str(env, "ftp://example.com/metadata.json");
            assert_eq!(validate_metadata(env, &metadata), Err(Error::UnsupportedProtocol));
        });
    }

    #[test]
    fn rejects_empty_description() {
        with_contract(|env| {
            let mut metadata = valid_metadata(env);
            metadata.description = Some(String::from_str(env, ""));
            assert_eq!(validate_metadata(env, &metadata), Err(Error::EmptyDescription));
        });
    }

    #[test]
    fn rejects_metadata_size_over_limit() {
        with_contract(|env| {
            set_max_metadata_size(env, 10).unwrap();
            let mut metadata = valid_metadata(env);
            metadata.metadata_uri = String::from_str(env, "ipfs://very-long-uri");
            assert_eq!(validate_metadata(env, &metadata), Err(Error::InvalidConfig));
        });
    }

    #[test]
    fn rejects_invalid_attribute_payload() {
        with_contract(|env| {
            let mut metadata = valid_metadata(env);
            metadata.attributes = Vec::from_array(
                env,
                [Attribute {
                    trait_type: String::from_str(env, ""),
                    value: String::from_str(env, "legendary"),
                }],
            );
            assert_eq!(validate_metadata(env, &metadata), Err(Error::InvalidURI));
        });
    }
}
