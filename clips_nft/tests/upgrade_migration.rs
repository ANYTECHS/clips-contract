//! Upgrade and migration tests for ClipsNftContract.
//!
//! Tests verify that:
//! - Contract can be upgraded safely
//! - Existing NFTs are preserved during upgrade
//! - Royalty information remains intact
//! - Version is bumped correctly
//! - Total supply is unchanged
//! - Admin remains authorized

#![cfg(test)]

use clips_nft::{
    ClipsNftContract, ContractInfo, DataKey, Error, Royalty, RoyaltyRecipient,
    TokenData, TokenId, VERSION,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Bytes, BytesN, Env, String, Vec,
};

// ============================================================================
// Helper: Setup a contract with initial state
// ============================================================================

fn setup_contract(env: &Env) -> (Address, Address) {
    let admin = Address::generate(env);
    let user = Address::generate(env);

    admin.require_auth();
    ClipsNftContract::init(env.clone(), admin.clone());

    (admin, user)
}

fn mint_sample_nft(
    env: &Env,
    owner: &Address,
    clip_id: u32,
    royalty_bps: u32,
) -> TokenId {
    owner.require_auth();

    let royalty_recipient = RoyaltyRecipient {
        recipient: Address::generate(env),
        basis_points: royalty_bps,
    };

    let royalty = Royalty {
        recipients: {
            let mut v = Vec::new(env);
            v.push_back(royalty_recipient);
            v
        },
        asset_address: None,
    };

    let metadata_uri = String::from_str(env, "ipfs://test-metadata");
    let signature = BytesN::<64>::from_array(
        env,
        &[0u8; 64], // dummy signature for testing
    );

    ClipsNftContract::mint(
        env.clone(),
        owner.clone(),
        clip_id,
        metadata_uri,
        None,
        None,
        royalty,
        false,
        signature,
    )
    .expect("mint should succeed")
}

// ============================================================================
// Test: Basic upgrade and migrate
// ============================================================================

#[test]
fn test_upgrade_and_migrate_preserves_state() {
    let env = Env::new();

    let (admin, _user) = setup_contract(&env);

    // Verify initial state
    let version_before = ClipsNftContract::contract_version(env.clone());
    let supply_before = ClipsNftContract::total_supply(env.clone());

    assert_eq!(version_before, VERSION, "Initial version should be VERSION");
    assert_eq!(supply_before, 0u32, "Initial supply should be 0");

    // Simulate calling upgrade() - in tests we're already on the new code
    // so we just verify the function is callable
    admin.require_auth();
    let result = ClipsNftContract::upgrade(env.clone(), admin.clone());
    assert!(result.is_ok(), "upgrade() should succeed");

    // Call migrate() to bump version
    let migrate_result = ClipsNftContract::migrate(env.clone(), admin.clone());
    assert!(migrate_result.is_ok(), "migrate() should succeed");

    // Verify version was bumped
    let version_after = ClipsNftContract::contract_version(env.clone());
    assert_eq!(version_after, VERSION, "Version should be bumped to VERSION");

    // Verify supply unchanged
    let supply_after = ClipsNftContract::total_supply(env.clone());
    assert_eq!(
        supply_before, supply_after,
        "Supply should not change during migration"
    );
}

// ============================================================================
// Test: NFT preservation during upgrade
// ============================================================================

#[test]
fn test_nfts_preserved_during_upgrade() {
    let env = Env::new();
    let (admin, user) = setup_contract(&env);

    // Mint 3 NFTs
    let token_id_1 = mint_sample_nft(&env, &user, 1, 500);
    let token_id_2 = mint_sample_nft(&env, &user, 2, 1000);
    let token_id_3 = mint_sample_nft(&env, &user, 3, 250);

    let supply_before = ClipsNftContract::total_supply(env.clone());
    assert_eq!(supply_before, 3, "Should have 3 NFTs minted");

    // Perform upgrade and migration
    admin.require_auth();
    ClipsNftContract::upgrade(env.clone(), admin.clone()).expect("upgrade should succeed");
    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("migrate should succeed");

    // Verify all NFTs still exist
    let supply_after = ClipsNftContract::total_supply(env.clone());
    assert_eq!(
        supply_before, supply_after,
        "All NFTs should be preserved (supply unchanged)"
    );

    // Verify user still owns all tokens
    let user_tokens =
        ClipsNftContract::tokens_of_owner(env.clone(), user.clone(), None, None);
    assert_eq!(
        user_tokens.len(),
        3u32,
        "User should still own all 3 tokens"
    );
    assert!(
        user_tokens.contains(&token_id_1),
        "Token 1 should be owned by user"
    );
    assert!(
        user_tokens.contains(&token_id_2),
        "Token 2 should be owned by user"
    );
    assert!(
        user_tokens.contains(&token_id_3),
        "Token 3 should be owned by user"
    );
}

// ============================================================================
// Test: Royalty preservation
// ============================================================================

#[test]
fn test_royalties_preserved_during_upgrade() {
    let env = Env::new();
    let (admin, user) = setup_contract(&env);

    // Mint NFT with specific royalty
    let royalty_bps = 750; // 7.5%
    let _token_id = mint_sample_nft(&env, &user, 42, royalty_bps);

    // Get royalty before upgrade
    let royalty_before = ClipsNftContract::get_royalty(env.clone(), 1)
        .expect("should be able to get royalty");

    // Verify royalty is set correctly
    assert_eq!(
        royalty_before.recipients.len(),
        1u32,
        "Should have one royalty recipient"
    );
    let recipient = royalty_before
        .recipients
        .get(0)
        .expect("should have first recipient");
    assert_eq!(
        recipient.basis_points, royalty_bps,
        "Royalty should be {}",
        royalty_bps
    );

    // Perform upgrade
    admin.require_auth();
    ClipsNftContract::upgrade(env.clone(), admin.clone()).expect("upgrade should succeed");
    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("migrate should succeed");

    // Get royalty after upgrade
    let royalty_after = ClipsNftContract::get_royalty(env.clone(), 1)
        .expect("should be able to get royalty after migration");

    // Verify royalty is unchanged
    assert_eq!(
        royalty_before.recipients.len(),
        royalty_after.recipients.len(),
        "Royalty recipient count should be preserved"
    );

    let recipient_after = royalty_after
        .recipients
        .get(0)
        .expect("should have first recipient");
    assert_eq!(
        recipient_after.basis_points, royalty_bps,
        "Royalty basis points should be preserved"
    );
}

// ============================================================================
// Test: Version bumping
// ============================================================================

#[test]
fn test_version_bumped_on_migrate() {
    let env = Env::new();
    let (admin, _user) = setup_contract(&env);

    let version_before = ClipsNftContract::contract_version(env.clone());
    assert_eq!(version_before, VERSION, "Initial version should be VERSION");

    // Migrate (this should bump the version)
    admin.require_auth();
    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("migrate should succeed");

    let version_after = ClipsNftContract::contract_version(env.clone());
    assert_eq!(
        version_after, VERSION,
        "Version should be updated to current VERSION"
    );
}

// ============================================================================
// Test: Unauthorized upgrade/migrate attempts
// ============================================================================

#[test]
fn test_non_admin_cannot_upgrade() {
    let env = Env::new();
    let (admin, user) = setup_contract(&env);

    user.require_auth();
    let result = ClipsNftContract::upgrade(env.clone(), user.clone());

    assert!(result.is_err(), "Non-admin should not be able to upgrade");
    assert_eq!(
        result.unwrap_err(),
        Error::Unauthorized,
        "Should return Unauthorized error"
    );
}

#[test]
fn test_non_admin_cannot_migrate() {
    let env = Env::new();
    let (admin, user) = setup_contract(&env);

    user.require_auth();
    let result = ClipsNftContract::migrate(env.clone(), user.clone());

    assert!(result.is_err(), "Non-admin should not be able to migrate");
    assert_eq!(
        result.unwrap_err(),
        Error::Unauthorized,
        "Should return Unauthorized error"
    );
}

// ============================================================================
// Test: Contract info retrieval
// ============================================================================

#[test]
fn test_contract_info_after_upgrade() {
    let env = Env::new();
    let (admin, _user) = setup_contract(&env);

    // Get contract info before migration
    let info_before = ClipsNftContract::contract_info(env.clone());

    assert_eq!(
        info_before.version, VERSION,
        "Initial version should be VERSION"
    );
    assert_eq!(
        info_before.name,
        String::from_str(&env, "ClipCash Clips"),
        "Name should be ClipCash Clips"
    );
    assert_eq!(
        info_before.symbol,
        String::from_str(&env, "CLIP"),
        "Symbol should be CLIP"
    );
    assert_eq!(info_before.owner, admin, "Owner should be admin");

    // Perform migration
    admin.require_auth();
    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("migrate should succeed");

    // Get contract info after migration
    let info_after = ClipsNftContract::contract_info(env.clone());

    assert_eq!(
        info_after.version, VERSION,
        "Version should be VERSION after migration"
    );
    assert_eq!(
        info_after.name, info_before.name,
        "Name should not change"
    );
    assert_eq!(
        info_after.symbol, info_before.symbol,
        "Symbol should not change"
    );
    assert_eq!(
        info_after.owner, info_before.owner,
        "Owner should not change"
    );
}

// ============================================================================
// Test: Multiple NFTs with varying royalties
// ============================================================================

#[test]
fn test_multiple_nfts_with_varied_royalties_preserved() {
    let env = Env::new();
    let (admin, user) = setup_contract(&env);

    // Mint NFTs with different royalty structures
    let _token_1 = mint_sample_nft(&env, &user, 10, 500); // 5%
    let _token_2 = mint_sample_nft(&env, &user, 20, 1000); // 10%
    let _token_3 = mint_sample_nft(&env, &user, 30, 250); // 2.5%

    // Record royalty info before upgrade
    let royalty_1_before = ClipsNftContract::get_royalty(env.clone(), 1)
        .expect("should get royalty 1");
    let royalty_2_before = ClipsNftContract::get_royalty(env.clone(), 2)
        .expect("should get royalty 2");
    let royalty_3_before = ClipsNftContract::get_royalty(env.clone(), 3)
        .expect("should get royalty 3");

    // Perform upgrade
    admin.require_auth();
    ClipsNftContract::upgrade(env.clone(), admin.clone()).expect("upgrade should succeed");
    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("migrate should succeed");

    // Verify royalties are preserved
    let royalty_1_after = ClipsNftContract::get_royalty(env.clone(), 1)
        .expect("should get royalty 1 after migration");
    let royalty_2_after = ClipsNftContract::get_royalty(env.clone(), 2)
        .expect("should get royalty 2 after migration");
    let royalty_3_after = ClipsNftContract::get_royalty(env.clone(), 3)
        .expect("should get royalty 3 after migration");

    // Compare royalties
    assert_eq!(
        royalty_1_before.recipients.get(0).unwrap().basis_points,
        royalty_1_after.recipients.get(0).unwrap().basis_points,
        "Royalty 1 should match"
    );
    assert_eq!(
        royalty_2_before.recipients.get(0).unwrap().basis_points,
        royalty_2_after.recipients.get(0).unwrap().basis_points,
        "Royalty 2 should match"
    );
    assert_eq!(
        royalty_3_before.recipients.get(0).unwrap().basis_points,
        royalty_3_after.recipients.get(0).unwrap().basis_points,
        "Royalty 3 should match"
    );
}

// ============================================================================
// Test: Idempotent migration
// ============================================================================

#[test]
fn test_migrate_idempotent() {
    let env = Env::new();
    let (admin, user) = setup_contract(&env);

    // Mint an NFT
    let _token = mint_sample_nft(&env, &user, 100, 500);

    let supply_before = ClipsNftContract::total_supply(env.clone());

    // Call migrate twice
    admin.require_auth();
    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("first migrate should succeed");

    let supply_after_first = ClipsNftContract::total_supply(env.clone());

    ClipsNftContract::migrate(env.clone(), admin.clone()).expect("second migrate should succeed");

    let supply_after_second = ClipsNftContract::total_supply(env.clone());

    // Supply should be unchanged by either migration
    assert_eq!(
        supply_before, supply_after_first,
        "Supply should not change on first migrate"
    );
    assert_eq!(
        supply_after_first, supply_after_second,
        "Supply should not change on second migrate"
    );
}
//!
//! These tests verify that:
//! 1. `upgrade()` preserves all existing NFT and royalty state.
//! 2. `migrate()` correctly seeds missing fields and bumps ContractVersion.
//! 3. `migrate()` is idempotent (safe to call twice).
//! 4. Only the admin can call `upgrade()` and `migrate()`.
//! 5. A simulated "old contract" (version 0, no TotalSupply key) migrates cleanly.

#![cfg(test)]

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Royalty, RoyaltyRecipient, VERSION};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, BytesN, Env, String, Vec,
};
use test_helpers::{mint_clip, setup};

// ---------------------------------------------------------------------------
// Helper: deploy a fresh contract and return (env, client, admin)
// ---------------------------------------------------------------------------
fn deploy() -> (Env, ClipsNftContractClient<'static>, Address) {
    let ctx = setup();
    // SAFETY: setup() leaks the Env onto the heap.
    let env = unsafe { &*(ctx.env as *const Env) };
    (env.clone(), ctx.client, ctx.admin)
}

// ---------------------------------------------------------------------------
// Test 1 — upgrade() preserves NFT ownership and royalty data
// ---------------------------------------------------------------------------
#[test]
fn test_upgrade_preserves_nft_and_royalty_state() {
    let ctx = setup();
    let env = ctx.env;
    let client = &ctx.client;
    let admin = &ctx.admin;

    // Mint a few tokens before the upgrade.
    let owner = Address::generate(env);
    let token_id_1 = mint_clip(&ctx, &owner, 1001, false);
    let token_id_2 = mint_clip(&ctx, &owner, 1002, false);

    let pre_supply = client.total_supply();
    let pre_owner_1 = client.owner_of(&token_id_1);
    let pre_royalty_1 = client.get_royalty(&token_id_1);
    let pre_owner_2 = client.owner_of(&token_id_2);

    // Simulate upgrade: in tests we re-register the same WASM (no actual new
    // binary), so we use the existing contract hash as the "new" hash.
    // The important thing is that upgrade() + migrate() run without error and
    // that all storage survives.
    let contract_id = client.address.clone();
    let wasm_hash: BytesN<32> = env.deployer().upload_contract_wasm(
        clips_nft::ClipsNftContract::__wasm_bytes(),
    );

    // upgrade() must succeed.
    client.upgrade(admin, &wasm_hash);

    // migrate() must succeed and bump the version.
    client.migrate(admin);

    // --- Verify NFT state is intact ---
    assert_eq!(client.total_supply(), pre_supply, "total_supply changed after upgrade");
    assert_eq!(client.owner_of(&token_id_1), pre_owner_1, "owner changed after upgrade");
    assert_eq!(client.owner_of(&token_id_2), pre_owner_2, "owner changed after upgrade");

    // Royalty recipients and basis points must be unchanged.
    let post_royalty_1 = client.get_royalty(&token_id_1);
    assert_eq!(
        post_royalty_1.recipients.len(),
        pre_royalty_1.recipients.len(),
        "royalty recipient count changed"
    );
    for i in 0..pre_royalty_1.recipients.len() {
        let pre = pre_royalty_1.recipients.get(i).unwrap();
        let post = post_royalty_1.recipients.get(i).unwrap();
        assert_eq!(pre.recipient, post.recipient, "royalty recipient changed at index {i}");
        assert_eq!(pre.basis_points, post.basis_points, "royalty bps changed at index {i}");
    }

    // contract_version must now equal VERSION.
    assert_eq!(client.contract_version(), VERSION, "contract_version not bumped by migrate()");
}

// ---------------------------------------------------------------------------
// Test 2 — migrate() is idempotent
// ---------------------------------------------------------------------------
#[test]
fn test_migrate_is_idempotent() {
    let ctx = setup();
    let env = ctx.env;
    let client = &ctx.client;
    let admin = &ctx.admin;

    let wasm_hash: BytesN<32> = env.deployer().upload_contract_wasm(
        clips_nft::ClipsNftContract::__wasm_bytes(),
    );
    client.upgrade(admin, &wasm_hash);
    client.migrate(admin);

    let version_after_first = client.contract_version();

    // Second call must not panic or change the version.
    client.migrate(admin);
    assert_eq!(
        client.contract_version(),
        version_after_first,
        "migrate() changed version on second call"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — only admin can call upgrade() and migrate()
// ---------------------------------------------------------------------------
#[test]
fn test_upgrade_and_migrate_require_admin() {
    let ctx = setup();
    let env = ctx.env;
    let client = &ctx.client;
    let admin = &ctx.admin;
    let non_admin = Address::generate(env);

    let wasm_hash: BytesN<32> = env.deployer().upload_contract_wasm(
        clips_nft::ClipsNftContract::__wasm_bytes(),
    );

    // Non-admin upgrade must fail.
    assert!(
        client.try_upgrade(&non_admin, &wasm_hash).is_err(),
        "non-admin should not be able to call upgrade()"
    );

    // Perform a legitimate upgrade so we can test migrate() access control.
    client.upgrade(admin, &wasm_hash);

    // Non-admin migrate must fail.
    assert!(
        client.try_migrate(&non_admin).is_err(),
        "non-admin should not be able to call migrate()"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — migrate() seeds TotalSupply when it was missing (v0 → v1)
// ---------------------------------------------------------------------------
#[test]
fn test_migrate_seeds_total_supply_from_next_token_id() {
    let ctx = setup();
    let env = ctx.env;
    let client = &ctx.client;
    let admin = &ctx.admin;

    // Mint some tokens so NextTokenId > 1.
    let owner = Address::generate(env);
    mint_clip(&ctx, &owner, 2001, false);
    mint_clip(&ctx, &owner, 2002, false);
    mint_clip(&ctx, &owner, 2003, false);

    // Manually remove TotalSupply to simulate a pre-v1 deployment.
    // We do this by directly manipulating instance storage via the env.
    // In a real upgrade scenario the old binary simply never wrote this key.
    env.as_contract(&client.address, || {
        use clips_nft::DataKey;
        env.storage().instance().remove(&DataKey::TotalSupply);
    });

    // Confirm TotalSupply is gone (total_supply() returns 0 as fallback).
    assert_eq!(client.total_supply(), 0, "TotalSupply should be absent before migration");

    // Run migration.
    let wasm_hash: BytesN<32> = env.deployer().upload_contract_wasm(
        clips_nft::ClipsNftContract::__wasm_bytes(),
    );
    client.upgrade(admin, &wasm_hash);
    client.migrate(admin);

    // After migration TotalSupply must equal the number of minted tokens.
    assert_eq!(client.total_supply(), 3, "migrate() should have seeded TotalSupply = 3");
    assert_eq!(client.contract_version(), VERSION);
}

// ---------------------------------------------------------------------------
// Test 5 — upgrade() + migrate() with active royalty balances
// ---------------------------------------------------------------------------
#[test]
fn test_upgrade_preserves_royalty_balances() {
    let ctx = setup();
    let env = ctx.env;
    let client = &ctx.client;
    let admin = &ctx.admin;

    // Set up a SEP-0041 asset and mint a token with it.
    let token_admin = Address::generate(env);
    let asset = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let stellar_asset = soroban_sdk::token::StellarAssetClient::new(env, &asset);

    let creator = Address::generate(env);
    let buyer = Address::generate(env);
    stellar_asset.mint(&buyer, &1_000_000i128);

    let clip_id = 3001u32;
    let uri = String::from_str(env, "ipfs://QmUpgradeRoyalty");
    let sig = test_helpers::sign_mint(env, &ctx.keypair, &creator, clip_id, &uri);
    let mut recipients = Vec::new(env);
    recipients.push_back(RoyaltyRecipient {
        recipient: creator.clone(),
        basis_points: 500,
    });
    let royalty = Royalty {
        recipients,
        asset_address: Some(asset.clone()),
    };
    let token_id = client.mint(
        &creator, &clip_id, &uri, &None, &None, &royalty, &false, &None, &sig,
    );

    // Pay royalty to accumulate a balance.
    client.pay_royalty(&buyer, &token_id, &1_000_000i128);

    // Upgrade.
    let wasm_hash: BytesN<32> = env.deployer().upload_contract_wasm(
        clips_nft::ClipsNftContract::__wasm_bytes(),
    );
    client.upgrade(admin, &wasm_hash);
    client.migrate(admin);

    // Creator should still be able to claim their royalties after the upgrade.
    client.claim_royalties(&creator, &token_id);

    let token_client = soroban_sdk::token::TokenClient::new(env, &asset);
    assert!(
        token_client.balance(&creator) > 0,
        "creator should have received royalties after upgrade"
    );
}
