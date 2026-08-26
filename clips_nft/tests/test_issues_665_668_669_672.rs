//! Integration tests for issues #665, #668, #669, #672.
//!
//! Covers:
//! - #665 — Creator address is assigned and persisted during mint.
//! - #668 — Thumbnail URI is stored and retrievable; URI validation enforced.
//! - #669 — Preview video URI is stored and retrievable; URI validation enforced.
//! - #672 — Royalty recipient mapping is stored per-token and updatable.

#![cfg(test)]

mod test_helpers;
use test_helpers::*;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Error, Royalty, RoyaltyRecipient};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

// ─── #665 — Creator assignment ────────────────────────────────────────────────

/// Minting a clip records the creator address on-chain.
#[test]
fn test_mint_persists_creator_address() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);

    let token_id = mint_clip(&ctx, &owner, 100, false);
    let stored_creator = ctx.client.get_creator(&token_id);

    // The mint helper passes owner as creator; verify it is persisted.
    assert_eq!(stored_creator, owner);
}

/// Two different tokens have independent creator records.
#[test]
fn test_creator_is_scoped_per_token() {
    let ctx = setup_test();
    let creator_a = Address::generate(ctx.env);
    let creator_b = Address::generate(ctx.env);

    let token_a = mint_clip(&ctx, &creator_a, 101, false);
    let token_b = mint_clip(&ctx, &creator_b, 102, false);

    assert_eq!(ctx.client.get_creator(&token_a), creator_a);
    assert_eq!(ctx.client.get_creator(&token_b), creator_b);
}

// ─── #668 — Thumbnail URI ─────────────────────────────────────────────────────

/// Minting with a valid IPFS thumbnail URI stores it correctly.
#[test]
fn test_mint_with_thumbnail_uri_stored() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let clip_id = 200u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta200");
    let thumbnail = String::from_str(ctx.env, "ipfs://QmThumb200");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);
    let royalty = default_royalty(ctx.env, owner.clone());

    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &Some(thumbnail.clone()),
        &None,
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert_eq!(ctx.client.get_thumbnail_uri(&token_id), Some(thumbnail));
}

/// Minting with an HTTPS thumbnail URI is accepted.
#[test]
fn test_thumbnail_uri_https_accepted() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let clip_id = 201u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta201");
    let thumbnail = String::from_str(ctx.env, "https://cdn.example.com/thumb.jpg");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);
    let royalty = default_royalty(ctx.env, owner.clone());

    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &Some(thumbnail.clone()),
        &None,
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert_eq!(ctx.client.get_thumbnail_uri(&token_id), Some(thumbnail));
}

/// Minting without a thumbnail results in None for that token.
#[test]
fn test_no_thumbnail_uri_returns_none() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);

    let token_id = mint_clip(&ctx, &owner, 202, false);
    assert_eq!(ctx.client.get_thumbnail_uri(&token_id), None);
}

/// Passing an unsupported-scheme thumbnail URI is rejected.
#[test]
fn test_invalid_thumbnail_uri_rejected() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let clip_id = 203u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta203");
    let bad_thumb = String::from_str(ctx.env, "ftp://bad.example.com/thumb.png");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);
    let royalty = default_royalty(ctx.env, owner.clone());

    let result = ctx.client.try_mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &Some(bad_thumb),
        &None,
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert!(result.is_err());
}

// ─── #669 — Preview video URI ─────────────────────────────────────────────────

/// Minting with a valid IPFS preview video URI stores it correctly.
#[test]
fn test_mint_with_preview_video_uri_stored() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let clip_id = 300u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta300");
    let preview = String::from_str(ctx.env, "ipfs://QmPreview300");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);
    let royalty = default_royalty(ctx.env, owner.clone());

    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &None,
        &Some(preview.clone()),
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert_eq!(ctx.client.get_preview_video_uri(&token_id), Some(preview));
}

/// Minting with an HTTPS preview video URI is accepted.
#[test]
fn test_preview_video_uri_https_accepted() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let clip_id = 301u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta301");
    let preview = String::from_str(ctx.env, "https://cdn.example.com/preview.mp4");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);
    let royalty = default_royalty(ctx.env, owner.clone());

    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &None,
        &Some(preview.clone()),
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert_eq!(ctx.client.get_preview_video_uri(&token_id), Some(preview));
}

/// Minting without a preview video results in None for that token.
#[test]
fn test_no_preview_video_uri_returns_none() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);

    let token_id = mint_clip(&ctx, &owner, 302, false);
    assert_eq!(ctx.client.get_preview_video_uri(&token_id), None);
}

/// Passing an unsupported-scheme preview video URI is rejected.
#[test]
fn test_invalid_preview_video_uri_rejected() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let clip_id = 303u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta303");
    let bad_preview = String::from_str(ctx.env, "ftp://bad.example.com/preview.mp4");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);
    let royalty = default_royalty(ctx.env, owner.clone());

    let result = ctx.client.try_mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &None,
        &Some(bad_preview),
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert!(result.is_err());
}

// ─── #672 — Royalty recipient mapping ─────────────────────────────────────────

/// Minting records the royalty recipient address for the token.
#[test]
fn test_mint_stores_royalty_recipient() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let royalty_addr = Address::generate(ctx.env);
    let clip_id = 400u32;
    let metadata_uri = String::from_str(ctx.env, "ipfs://QmMeta400");
    let sig = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id, &metadata_uri);

    let mut recipients = soroban_sdk::Vec::new(ctx.env);
    recipients.push_back(RoyaltyRecipient {
        recipient: royalty_addr.clone(),
        basis_points: 750,
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };

    let token_id = ctx.client.mint(
        &owner,
        &clip_id,
        &metadata_uri,
        &None,
        &None,
        &royalty,
        &false,
        &None,
        &sig,
    );

    assert_eq!(ctx.client.get_royalty_recipient(&token_id), royalty_addr);
}

/// Two tokens have independent royalty recipient records.
#[test]
fn test_royalty_recipient_scoped_per_token() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let recipient_a = Address::generate(ctx.env);
    let recipient_b = Address::generate(ctx.env);

    let clip_id_a = 401u32;
    let clip_id_b = 402u32;
    let uri_a = String::from_str(ctx.env, "ipfs://QmMeta401");
    let uri_b = String::from_str(ctx.env, "ipfs://QmMeta402");
    let sig_a = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id_a, &uri_a);
    let sig_b = sign_mint(ctx.env, &ctx.keypair, &owner, clip_id_b, &uri_b);

    let mut rec_a = soroban_sdk::Vec::new(ctx.env);
    rec_a.push_back(RoyaltyRecipient {
        recipient: recipient_a.clone(),
        basis_points: 500,
    });
    let mut rec_b = soroban_sdk::Vec::new(ctx.env);
    rec_b.push_back(RoyaltyRecipient {
        recipient: recipient_b.clone(),
        basis_points: 300,
    });

    let token_a = ctx.client.mint(
        &owner,
        &clip_id_a,
        &uri_a,
        &None,
        &None,
        &Royalty {
            recipients: rec_a,
            asset_address: None,
        },
        &false,
        &None,
        &sig_a,
    );
    let token_b = ctx.client.mint(
        &owner,
        &clip_id_b,
        &uri_b,
        &None,
        &None,
        &Royalty {
            recipients: rec_b,
            asset_address: None,
        },
        &false,
        &None,
        &sig_b,
    );

    assert_eq!(ctx.client.get_royalty_recipient(&token_a), recipient_a);
    assert_eq!(ctx.client.get_royalty_recipient(&token_b), recipient_b);
}

/// Admin can update the royalty recipient after minting.
#[test]
fn test_royalty_recipient_update_by_admin() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let new_recipient = Address::generate(ctx.env);

    let token_id = mint_clip(&ctx, &owner, 403, false);

    // Admin updates the mapping
    ctx.client
        .update_royalty_recipient(&ctx.admin, &token_id, &new_recipient);

    assert_eq!(ctx.client.get_royalty_recipient(&token_id), new_recipient);
}
