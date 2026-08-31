//! Mint validator — validates mint requests before NFT creation.
//!
//! Pre-validates every mint request included in a batch before processing begins:
//! 1. Owner address validity & blacklist check
//! 2. Clip ID on-chain uniqueness
//! 3. Metadata URI presence & scheme validity (including thumbnail & preview URIs)
//! 4. Royalty percentage basis points & recipient address
//! 5. Duplicate clip IDs within the batch

use soroban_sdk::{Address, Env, String, Vec};

use crate::clip_id_storage;
use crate::metadata_uri_builder::validate_uri;
use crate::mint_request::{BatchMintRequest, MintRequest};
use crate::royalty_recipient_validator;
use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::token_owner_storage;
use crate::types::{DataKey, Error, Royalty, RoyaltyRecipient};

/// Validate a single mint request before any state is written.
pub fn validate_mint(
    env: &Env,
    clip_id: u32,
    metadata_uri: &String,
    royalty: &Royalty,
    owner: &Address,
) -> Result<(), Error> {
    if env
        .storage()
        .persistent()
        .has(&DataKey::ClipIdMinted(clip_id))
    {
        return Err(Error::ClipAlreadyMinted);
    }

    if metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }

    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(owner.clone()))
        .unwrap_or(false)
    {
        return Err(Error::Unauthorized);
    }

    if royalty.recipients.is_empty() {
        return Err(Error::InvalidBasisPoints);
    }
    let mut total_bps: u32 = 0;
    for r in royalty.recipients.iter() {
        if r.basis_points > MAX_ROYALTY_BPS {
            return Err(Error::InvalidBasisPoints);
        }
        total_bps = total_bps.saturating_add(r.basis_points);
    }
    if total_bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }

    Ok(())
}

/// Validate a single [`MintRequest`] (Owner, Clip ID, Metadata URI, Royalties).
pub fn validate_mint_request(env: &Env, request: &MintRequest) -> Result<(), Error> {
    // 1. Validate Owner (valid address format & blacklist check)
    token_owner_storage::validate_owner(env, &request.owner)?;
    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(request.owner.clone()))
        .unwrap_or(false)
    {
        return Err(Error::Unauthorized);
    }

    if let Some(ref creator) = request.creator_address {
        token_owner_storage::validate_owner(env, creator)?;
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Blacklisted(creator.clone()))
            .unwrap_or(false)
        {
            return Err(Error::Unauthorized);
        }
    }

    // 2. Validate Clip ID (on-chain duplicate guard)
    //
    // Optimization: `clip_id_storage::is_clip_mapped(env, request.clip_id)`
    // already performs `storage().persistent().has(DataKey::ClipIdMinted(id))`.
    // The previous code OR'd it with a second, identical `.has()` call —
    // producing two redundant ledger lookups per validated NFT. A single
    // lookup is equivalent and sufficient.
    //
    // Savings: −1 persistent read per mint request.
    if clip_id_storage::is_clip_mapped(env, request.clip_id) {
        return Err(Error::ClipAlreadyMinted);
    }

    // 3. Validate Metadata URI (non-empty, valid scheme for main URI, thumbnail, preview)
    if request.metadata_uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    validate_uri(&request.metadata_uri)?;

    if let Some(ref thumb) = request.thumbnail_uri {
        validate_uri(thumb)?;
    }
    if let Some(ref preview) = request.preview_video_uri {
        validate_uri(preview)?;
    }

    // 4. Validate Royalties (basis points limit & recipients)
    if request.royalty_info.recipients.is_empty() {
        return Err(Error::InvalidBasisPoints);
    }
    let mut total_bps: u32 = 0;
    for r in request.royalty_info.recipients.iter() {
        if r.basis_points > MAX_ROYALTY_BPS {
            return Err(Error::InvalidBasisPoints);
        }
        total_bps = total_bps.saturating_add(r.basis_points);
        royalty_recipient_validator::validate_royalty_recipient(env, &r.recipient)?;
    }
    if total_bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }

    Ok(())
}

/// Validate every mint request included in a batch before processing begins.
///
/// Acceptance Criteria:
/// - Validate Owner for every request
/// - Validate Clip ID for every request (on-chain check)
/// - Validate Metadata URI for every request
/// - Validate Royalties for every request
/// - Validate Duplicate clips (both within the batch and on-chain)
///
/// Aborts batch (returns Err) immediately if validation fails for any request.
pub fn validate_batch_mint(env: &Env, batch: &BatchMintRequest) -> Result<(), Error> {
    // 1. Read configured limit & validate request size
    batch.validate_against_env(env)?;

    let mut seen_clips = Vec::new(env);

    for request in batch.requests.iter() {
        // Validate duplicate clips within the batch
        if seen_clips.contains(&request.clip_id) {
            return Err(Error::ClipAlreadyMinted);
        }
        seen_clips.push_back(request.clip_id);

        // Validate Owner, Clip ID, Metadata URI, Royalties
        validate_mint_request(env, &request)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyRecipient;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

    fn env_with_clip(clip_id: u32) -> Env {
        let env = Env::default();
        env.storage()
            .persistent()
            .set(&DataKey::ClipIdMinted(clip_id), &true);
        env
    }

    #[test]
    fn valid_mint_passes() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: creator.clone(),
                    basis_points: 500
                }
            ],
            asset_address: None,
        };
        let royalty = Royalty { recipients: soroban_sdk::vec![&env, RoyaltyRecipient { recipient: creator.clone(), basis_points: 500 }], asset_address: None };
        assert!(validate_mint(&env, 1, &uri, &royalty, &creator).is_ok());
    }

    #[test]
    fn duplicate_clip_fails() {
        let env = env_with_clip(42);
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: creator.clone(),
                    basis_points: 500
                }
            ],
            asset_address: None,
        };
        assert_eq!(
            validate_mint(&env, 42, &uri, &royalty, &creator),
            Err(Error::ClipAlreadyMinted)
        );
        let royalty = Royalty { recipients: soroban_sdk::vec![&env, RoyaltyRecipient { recipient: creator.clone(), basis_points: 500 }], asset_address: None };
        assert_eq!(validate_mint(&env, 42, &uri, &royalty, &creator), Err(Error::ClipAlreadyMinted));
    }

    #[test]
    fn empty_metadata_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let uri = String::from_str(&env, "");
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: creator.clone(),
                    basis_points: 500
                }
            ],
            asset_address: None,
        };
        assert_eq!(
            validate_mint(&env, 1, &uri, &royalty, &creator),
            Err(Error::InvalidURI)
        );
        let royalty = Royalty { recipients: soroban_sdk::vec![&env, RoyaltyRecipient { recipient: creator.clone(), basis_points: 500 }], asset_address: None };
        assert_eq!(validate_mint(&env, 1, &uri, &royalty, &creator), Err(Error::InvalidURI));
    }

    #[test]
    fn blacklisted_wallet_fails() {
        let env = Env::default();
        let creator = Address::generate(&env);
        env.storage()
            .persistent()
            .set(&DataKey::Blacklisted(creator.clone()), &true);
        let uri = String::from_str(&env, "ipfs://QmTest");
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: creator.clone(),
                    basis_points: 500
                }
            ],
            asset_address: None,
        };
        assert_eq!(
            validate_mint(&env, 1, &uri, &royalty, &creator),
            Err(Error::Unauthorized)
        );
        let royalty = Royalty { recipients: soroban_sdk::vec![&env, RoyaltyRecipient { recipient: creator.clone(), basis_points: 500 }], asset_address: None };
        assert_eq!(validate_mint(&env, 1, &uri, &royalty, &creator), Err(Error::Unauthorized));
    }

    #[test]
    fn validate_batch_mint_detects_within_batch_duplicates() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient,
                    basis_points: 500
                }
            ],
            asset_address: None,
        };
        let royalty = Royalty { recipients: soroban_sdk::vec![&env, RoyaltyRecipient { recipient, basis_points: 500 }], asset_address: None };

        let req1 = MintRequest {
            clip_id: 10,
            owner: owner.clone(),
            creator: owner.clone(),
            metadata_uri: String::from_str(&env, "ipfs://Qm1"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: royalty.clone(),
            creator_address: None,
            creator_display_name: None,
        };
        let req2 = MintRequest {
            clip_id: 10, // Duplicate clip_id inside batch!
            owner: owner.clone(),
            creator: owner.clone(),
            metadata_uri: String::from_str(&env, "ipfs://Qm2"),
            thumbnail_uri: None,
            preview_video_uri: None,
            royalty_info: royalty.clone(),
            creator_address: None,
            creator_display_name: None,
        };

        let batch = BatchMintRequest {
            requests: Vec::from_array(&env, [req1, req2]),
        };

        assert_eq!(
            validate_batch_mint(&env, &batch),
            Err(Error::ClipAlreadyMinted)
        );
    }
}
