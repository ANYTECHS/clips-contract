//! Mint request validator — validates every required field of an NFT mint
//! request.
//!
//! Runs four domain checks in order, stopping at the first failure:
//!
//! 1. [`validate_token_data`] — clip id & clip uniqueness.
//! 2. [`validate_creator`] — creator address eligibility.
//! 3. [`validate_metadata`] — metadata URI presence and scheme.
//! 4. [`validate_recipient`] — owner/recipient eligibility.

use soroban_sdk::{Address, Env, String};

use crate::blacklist;
use crate::clip_id_storage;
use crate::metadata_uri_builder::validate_uri;
use crate::mint_request::MintRequest;
use crate::types::{DataKey, Error};
use crate::validators::{ValidationContext, ValidationResult, Validator};

// ─── Input type ──────────────────────────────────────────────────────────────

/// Input accepted by [`MintRequestValidator`]: a single NFT mint request.
pub type MintRequestInput = MintRequest;

// ─── Validator ───────────────────────────────────────────────────────────────

/// Validator for a single NFT mint request.
///
/// Implements the standardized [`Validator`] interface so the check is reusable
/// across entry points and always returns a [`ValidationResult`].
pub struct MintRequestValidator;

impl Validator<MintRequestInput> for MintRequestValidator {
    fn validate(
        &self,
        ctx: &ValidationContext<'_>,
        request: &MintRequestInput,
    ) -> ValidationResult {
        let _ = ctx;
        validate_token_data(ctx.env, request)
            .and_then(|()| validate_creator(ctx.env, request))
            .and_then(|()| validate_metadata(ctx.env, request))
            .and_then(|()| validate_recipient(ctx.env, request))
            .into()
    }
}

// ─── Domain checks ───────────────────────────────────────────────────────────

/// Validate the token data of a mint request.
///
/// - `clip_id` must not be the reserved sentinel value `0`.
/// - `clip_id` must not already be mapped to a minted token.
///
/// # Errors
/// - [`Error::InvalidTokenId`] — `clip_id` is `0`.
/// - [`Error::ClipAlreadyMinted`] — `clip_id` is already mapped on-chain.
pub fn validate_token_data(env: &Env, request: &MintRequestInput) -> Result<(), Error> {
    if request.clip_id == 0 {
        return Err(Error::InvalidTokenId);
    }
    if clip_id_storage::is_clip_mapped(env, request.clip_id) {
        return Err(Error::ClipAlreadyMinted);
    }
    Ok(())
}

/// Validate the creator information of a mint request.
///
/// The creator address must not be the contract's own address; the contract
/// cannot meaningfully act as a creator. Falls back to `request.creator` when
/// no explicit `creator_address` is supplied.
///
/// # Errors
/// - [`Error::InvalidAddress`] — creator is the contract itself.
pub fn validate_creator(env: &Env, request: &MintRequestInput) -> Result<(), Error> {
    let creator = request
        .creator_address
        .as_ref()
        .unwrap_or(&request.creator);
    if *creator == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate the metadata of a mint request.
///
/// - `metadata_uri` must be non-empty and use a supported scheme (`ipfs://`,
///   `https://`, `ar://`).
/// - `thumbnail_uri` and `preview_video_uri`, when present, must use a
///   supported scheme.
///
/// # Errors
/// - [`Error::InvalidURI`] — URI is empty or uses an unsupported scheme.
pub fn validate_metadata(env: &Env, request: &MintRequestInput) -> Result<(), Error> {
    let _ = env;
    validate_uri(&request.metadata_uri)?;
    if let Some(thumb) = &request.thumbnail_uri {
        validate_uri(thumb)?;
    }
    if let Some(preview) = &request.preview_video_uri {
        validate_uri(preview)?;
    }
    Ok(())
}

/// Validate the recipient (owner) information of a mint request.
///
/// - The owner must not be the contract's own address.
/// - The owner must not be blacklisted.
///
/// # Errors
/// - [`Error::InvalidRecipient`] — owner is the contract itself.
/// - [`Error::Unauthorized`] — owner is blacklisted.
pub fn validate_recipient(env: &Env, request: &MintRequestInput) -> Result<(), Error> {
    if request.owner == env.current_contract_address() {
        return Err(Error::InvalidRecipient);
    }
    if blacklist::is_blacklisted(env, &request.owner) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

// ─── Combined entry point ────────────────────────────────────────────────────

/// Validate every required field of a mint request.
///
/// Equivalent to [`MintRequestValidator`] but exposed as a plain function for
/// call sites that do not use the standardized [`Validator`] interface.
///
/// # Errors
/// Returns the first enclosing [`Error`] from any failed domain check.
pub fn validate_mint_request(env: &Env, request: &MintRequestInput) -> Result<(), Error> {
    validate_token_data(env, request)?;
    validate_creator(env, request)?;
    validate_metadata(env, request)?;
    validate_recipient(env, request)?;
    Ok(())
}

// ─── Address helpers ─────────────────────────────────────────────────────────

/// Return the effective owner address of a mint request.
pub fn owner_address(request: &MintRequestInput) -> &Address {
    &request.owner
}

/// Return the effective creator address of a mint request.
pub fn creator_address(request: &MintRequestInput) -> &Address {
    request
        .creator_address
        .as_ref()
        .unwrap_or(&request.creator)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Env, Vec};

    fn valid_request(env: &Env) -> MintRequestInput {
        let owner = Address::generate(env);
        let creator = Address::generate(env);
        MintRequestInput {
            clip_id: 1,
            owner: owner.clone(),
            creator: creator.clone(),
            metadata_uri: String::from_str(env, "ipfs://QmValid"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: crate::types::Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    crate::types::RoyaltyRecipient {
                        recipient: creator.clone(),
                        basis_points: 500,
                    },
                ],
                asset_address: None,
            },
            creator_address: None,
            creator_display_name: None,
        }
    }

    #[test]
    fn accepts_valid_mint_request() {
        let env = Env::default();
        let request = valid_request(&env);
        assert!(validate_mint_request(&env, &request).is_ok());
        assert!(MintRequestValidator
            .validate(&ValidationContext::new(&env), &request)
            .is_valid());
    }

    #[test]
    fn rejects_zero_clip_id() {
        let env = Env::default();
        let mut request = valid_request(&env);
        request.clip_id = 0;
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::InvalidTokenId)
        );
    }

    #[test]
    fn rejects_already_minted_clip() {
        let env = Env::default();
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(1), &true);
        let request = valid_request(&env);
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::ClipAlreadyMinted)
        );
    }

    #[test]
    fn rejects_contract_as_creator() {
        let env = Env::default();
        let mut request = valid_request(&env);
        request.creator_address = Some(env.current_contract_address());
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::InvalidAddress)
        );
    }

    #[test]
    fn rejects_empty_metadata_uri() {
        let env = Env::default();
        let mut request = valid_request(&env);
        request.metadata_uri = String::from_str(&env, "");
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::InvalidURI)
        );
    }

    #[test]
    fn rejects_unsupported_metadata_scheme() {
        let env = Env::default();
        let mut request = valid_request(&env);
        request.metadata_uri = String::from_str(&env, "ftp://example.com/meta");
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::InvalidURI)
        );
    }

    #[test]
    fn rejects_contract_as_owner() {
        let env = Env::default();
        let mut request = valid_request(&env);
        request.owner = env.current_contract_address();
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::InvalidRecipient)
        );
    }

    #[test]
    fn rejects_blacklisted_owner() {
        let env = Env::default();
        let mut request = valid_request(&env);
        blacklist::add_wallet(&env, &request.owner);
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn stops_at_first_failure() {
        let env = Env::default();
        let mut request = valid_request(&env);
        request.clip_id = 0;
        request.metadata_uri = String::from_str(&env, "");
        // token data is checked before metadata
        assert_eq!(
            validate_mint_request(&env, &request),
            Err(Error::InvalidTokenId)
        );
    }

    #[test]
    fn owner_and_creator_address_helpers() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);
        let request = MintRequestInput {
            clip_id: 1,
            owner: owner.clone(),
            creator: creator.clone(),
            metadata_uri: String::from_str(&env, "ipfs://QmValid"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: crate::types::Royalty {
                recipients: soroban_sdk::vec![
                    &env,
                    crate::types::RoyaltyRecipient {
                        recipient: creator.clone(),
                        basis_points: 500,
                    },
                ],
                asset_address: None,
            },
            creator_address: Some(creator.clone()),
            creator_display_name: None,
        };
        assert_eq!(owner_address(&request), &owner);
        assert_eq!(creator_address(&request), &creator);
    }
}
