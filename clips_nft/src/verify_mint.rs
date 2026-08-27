//! Post-mint metadata verification (issue #678).
//!
//! Verifies that all metadata records (metadata record, URI, creator, royalty data)
//! are successfully stored after a mint transaction.

use soroban_sdk::{Env, Address, String};

use crate::types::{DataKey, Error, TokenId, Royalty};
use crate::mint_request::MintRequest;
use crate::{token_storage, creator_storage, royalty_recipient, royalty_percentage};

/// Perform post-mint verification on all persisted metadata.
///
/// Returns `Err(Error::CorruptedStorage)` if any expected data is missing or incorrect.
pub fn verify_post_mint(
    env: &Env,
    token_id: TokenId,
    request: &MintRequest,
) -> Result<(), Error> {
    // 1. Verify URI
    let stored_uri = token_storage::get_metadata(env, token_id)?;
    if stored_uri != request.metadata_uri {
        return Err(Error::CorruptedStorage);
    }

    // 2. Verify metadata record (MetadataIndex maps to this token_id)
    let indexed_token_id: TokenId = env
        .storage()
        .persistent()
        .get(&DataKey::MetadataIndex(request.metadata_uri.clone()))
        .ok_or(Error::CorruptedStorage)?;
    if indexed_token_id != token_id {
        return Err(Error::CorruptedStorage);
    }

    // Verify metadata record exists if it is registered in MetadataRecord
    if env.storage().persistent().has(&DataKey::MetadataRecord(request.metadata_uri.clone())) {
        let exists: bool = env
            .storage()
            .persistent()
            .get(&DataKey::MetadataRecord(request.metadata_uri.clone()))
            .unwrap_or(false);
        if !exists {
            return Err(Error::CorruptedStorage);
        }
    }

    // 3. Verify creator
    let expected_creator = request.creator_address.clone().unwrap_or_else(|| request.owner.clone());
    let stored_creator = creator_storage::get_creator(env, token_id)?;
    if stored_creator != expected_creator {
        return Err(Error::CorruptedStorage);
    }

    // 4. Verify royalty data
    let stored_royalty = token_storage::get_royalty(env, token_id)?;
    if stored_royalty != request.royalty_info {
        return Err(Error::CorruptedStorage);
    }

    let total_bps: u32 = request.royalty_info.recipients.iter().map(|r| r.basis_points).sum();
    let stored_percentage = royalty_percentage::get_royalty_percentage(env, token_id)?;
    if stored_percentage != total_bps {
        return Err(Error::CorruptedStorage);
    }

    let stored_recipient = royalty_recipient::get_royalty_recipient(env, token_id)?;
    if stored_recipient != request.royalty_info.recipients.get(0).unwrap().recipient {
        return Err(Error::CorruptedStorage);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, String};
    use crate::types::{TokenData, Royalty, RoyaltyRecipient};

    fn test_env() -> Env {
        Env::default()
    }

    fn make_request(env: &Env, owner: &Address, creator: &Address) -> MintRequest {
        MintRequest {
            owner: owner.clone(),
            clip_id: 100,
            metadata_uri: String::from_str(env, "ipfs://QmTest"),
            royalty_info: Royalty {
                recipients: soroban_sdk::vec![env, RoyaltyRecipient { recipient: creator.clone(), basis_points: 500 }],
                asset_address: None,
            },
            creator: creator.clone(),
            creator_address: Some(creator.clone()),
            creator_display_name: Some(String::from_str(env, "Creator")),
            thumbnail_uri: None,
            preview_video_uri: None,
        }
    }

    #[test]
    fn test_verification_success() {
        let env = test_env();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);
        let req = make_request(&env, &owner, &creator);
        let token_id = 1u32;

        // Set up mock storage
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &TokenData { owner: owner.clone(), clip_id: req.clip_id });
        token_storage::set_metadata(&env, token_id, &req.metadata_uri).unwrap();
        creator_storage::set_creator_with_name(&env, token_id, &creator, req.creator_display_name.clone());
        token_storage::set_royalty(&env, token_id, &req.royalty_info);
        let total_bps: u32 = req.royalty_info.recipients.iter().map(|r| r.basis_points).sum();
        royalty_percentage::set_royalty_percentage(&env, token_id, total_bps).unwrap();
        royalty_recipient::set_royalty_recipient(&env, token_id, &req.royalty_info.recipients.get(0).unwrap().recipient);

        assert!(verify_post_mint(&env, token_id, &req).is_ok());
    }

    #[test]
    fn test_verification_fails_on_uri_mismatch() {
        let env = test_env();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);
        let req = make_request(&env, &owner, &creator);
        let token_id = 1u32;

        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &TokenData { owner: owner.clone(), clip_id: req.clip_id });
        token_storage::set_metadata(&env, token_id, &String::from_str(&env, "ipfs://QmWrong")).unwrap();
        creator_storage::set_creator_with_name(&env, token_id, &creator, req.creator_display_name.clone());
        token_storage::set_royalty(&env, token_id, &req.royalty_info);
        let total_bps: u32 = req.royalty_info.recipients.iter().map(|r| r.basis_points).sum();
        royalty_percentage::set_royalty_percentage(&env, token_id, total_bps).unwrap();
        royalty_recipient::set_royalty_recipient(&env, token_id, &req.royalty_info.recipients.get(0).unwrap().recipient);

        assert_eq!(verify_post_mint(&env, token_id, &req), Err(Error::CorruptedStorage));
    }

    #[test]
    fn test_verification_fails_on_creator_mismatch() {
        let env = test_env();
        let owner = Address::generate(&env);
        let creator = Address::generate(&env);
        let other_creator = Address::generate(&env);
        let req = make_request(&env, &owner, &creator);
        let token_id = 1u32;

        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &TokenData { owner: owner.clone(), clip_id: req.clip_id });
        token_storage::set_metadata(&env, token_id, &req.metadata_uri).unwrap();
        creator_storage::set_creator_with_name(&env, token_id, &other_creator, req.creator_display_name.clone());
        token_storage::set_royalty(&env, token_id, &req.royalty_info);
        let total_bps: u32 = req.royalty_info.recipients.iter().map(|r| r.basis_points).sum();
        royalty_percentage::set_royalty_percentage(&env, token_id, total_bps).unwrap();
        royalty_recipient::set_royalty_recipient(&env, token_id, &req.royalty_info.recipients.get(0).unwrap().recipient);

        assert_eq!(verify_post_mint(&env, token_id, &req), Err(Error::CorruptedStorage));
    }
}
