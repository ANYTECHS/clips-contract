// Test suite for image and animation_url metadata fields
// Verifies OpenSea metadata standard compliance

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, BytesN, Env, String, Vec,
};

mod test_helpers;
use test_helpers::*;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Error, Royalty, RoyaltyRecipient};

#[test]
fn test_mint_with_image_and_animation_url() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 1u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata");
    let image = Some(String::from_str(ctx.env, "https://example.com/image.png"));
    let animation_url = Some(String::from_str(ctx.env, "ipfs://QmAnimation.mp4"));
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &uri,
        &image,
        &animation_url,
        &royalty,
        &false,
        &sig
    );
    
    assert_eq!(token_id, 1);
    
    // Verify JSON output includes both fields
    let json = ctx.client.get_metadata_json(&token_id);
    assert!(json.contains(&String::from_str(ctx.env, "\"image\":")));
    assert!(json.contains(&String::from_str(ctx.env, "\"animation_url\":")));
}

#[test]
fn test_mint_with_only_animation_url() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 2u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata2");
    let animation_url = Some(String::from_str(ctx.env, "https://example.com/video.mp4"));
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &uri,
        &None,  // No image
        &animation_url,
        &royalty,
        &false,
        &sig
    );
    
    assert_eq!(token_id, 1);
    
    // Verify JSON output includes animation_url but not image
    let json = ctx.client.get_metadata_json(&token_id);
    assert!(!json.contains(&String::from_str(ctx.env, "\"image\":")));
    assert!(json.contains(&String::from_str(ctx.env, "\"animation_url\":")));
}

#[test]
fn test_mint_without_media_fields() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 3u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata3");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &uri,
        &None,
        &None,
        &royalty,
        &false,
        &sig
    );
    
    assert_eq!(token_id, 1);
    
    // Verify JSON output excludes both fields
    let json = ctx.client.get_metadata_json(&token_id);
    assert!(!json.contains(&String::from_str(ctx.env, "\"image\":")));
    assert!(!json.contains(&String::from_str(ctx.env, "\"animation_url\":")));
}

#[test]
fn test_mint_with_invalid_image_url_fails() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 4u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata4");
    let invalid_image = Some(String::from_str(ctx.env, "ftp://invalid.com/image.png"));
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let result = ctx.client.try_mint(
        &owner,
        &clip_id,
        &uri,
        &invalid_image,
        &None,
        &royalty,
        &false,
        &sig
    );
    
    assert_eq!(result, Err(Ok(Error::InvalidImageUrl)));
}

#[test]
fn test_mint_with_invalid_animation_url_fails() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 5u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata5");
    let invalid_animation = Some(String::from_str(ctx.env, "http://insecure.com/video.mp4"));
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let result = ctx.client.try_mint(
        &owner,
        &clip_id,
        &uri,
        &None,
        &invalid_animation,
        &royalty,
        &false,
        &sig
    );
    
    assert_eq!(result, Err(Ok(Error::InvalidAnimationUrl)));
}

#[test]
fn test_refresh_metadata_updates_image() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let token_id = mint_clip(&ctx, &owner, 6, false);
    
    let new_image = Some(String::from_str(ctx.env, "https://example.com/new-image.png"));
    ctx.client.refresh_metadata(&ctx.admin, &token_id, &None, &new_image, &None);
    
    let json = ctx.client.get_metadata_json(&token_id);
    assert!(json.contains(&String::from_str(ctx.env, "new-image.png")));
}

#[test]
fn test_refresh_metadata_updates_animation_url() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let token_id = mint_clip(&ctx, &owner, 7, false);
    
    let new_animation = Some(String::from_str(ctx.env, "ipfs://QmNewAnimation.webm"));
    ctx.client.refresh_metadata(&ctx.admin, &token_id, &None, &None, &new_animation);
    
    let json = ctx.client.get_metadata_json(&token_id);
    assert!(json.contains(&String::from_str(ctx.env, "QmNewAnimation.webm")));
}

#[test]
fn test_refresh_metadata_clears_image_with_empty_string() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 8u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata8");
    let image = Some(String::from_str(ctx.env, "https://example.com/original.png"));
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &uri,
        &image,
        &None,
        &royalty,
        &false,
        &sig
    );
    
    // Clear the image field
    let empty = Some(String::from_str(ctx.env, ""));
    ctx.client.refresh_metadata(&ctx.admin, &token_id, &None, &empty, &None);
    
    let json = ctx.client.get_metadata_json(&token_id);
    assert!(!json.contains(&String::from_str(ctx.env, "\"image\":")));
}

#[test]
fn test_refresh_metadata_with_invalid_image_url_fails() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let token_id = mint_clip(&ctx, &owner, 9, false);
    
    let invalid_image = Some(String::from_str(ctx.env, "ftp://invalid.com/image.png"));
    let result = ctx.client.try_refresh_metadata(
        &ctx.admin,
        &token_id,
        &None,
        &invalid_image,
        &None
    );
    
    assert_eq!(result, Err(Ok(Error::InvalidImageUrl)));
}

#[test]
fn test_ipfs_urls_are_valid() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    
    let clip_id = 10u32;
    let uri = String::from_str(ctx.env, "ipfs://QmMetadata10");
    let image = Some(String::from_str(ctx.env, "ipfs://QmImage"));
    let animation_url = Some(String::from_str(ctx.env, "ipfs://QmAnimation"));
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &uri);
    let royalty = default_royalty(ctx.env, owner.clone());
    
    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &uri,
        &image,
        &animation_url,
        &royalty,
        &false,
        &sig
    );
    
    assert_eq!(token_id, 1);
}
