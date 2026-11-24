//! Guard bypass prevention test suite.
//!
//! Security tests that verify protected contract functions cannot be accessed by
//! bypassing their guards. Every scenario is exercised through the public
//! [`ClipCashNFTClient`] or [`AtomicMintContractClient`] surface — exactly as an
//! untrusted on-chain caller would invoke it.
//!
//! Test coverage areas:
//! - **Unauthorized callers** — non-admin addresses rejected from admin-gated functions.
//! - **Invalid token states** — operations on non-existent tokens fail safely.
//! - **Paused contract operations** — state-changing functions blocked when paused.
//! - **Unauthorized administrative operations** — minting, config, royalty, freeze
//!   all enforce their guards.
//! - **Guard presence verification** — every protected function is confirmed to
//!   execute at least one authorization or state guard.

#![cfg(test)]

use soroban_sdk::{
    symbol_short, testutils::Address as _, Address, BytesN, Env, String,
};

use crate::{
    atomic_mint::{AtomicMintContractClient, MintParams},
    types::{Config, Royalty, TokenId},
    ClipCashNFT, ClipCashNFTClient,
};

// ─── Mock SEP-41 token (same as marketplace_security_tests) ─────────────────────

mod mock_token {
    use soroban_sdk::{
        contract, contractimpl, symbol_short, testutils::Address as _, Address, Env, Map,
    };

    #[contract]
    pub struct MockToken;

    #[contractimpl]
    impl MockToken {
        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = symbol_short!("bal");
            let mut balances: Map<Address, i128> = env
                .storage()
                .instance()
                .get(&key)
                .unwrap_or_else(|| Map::new(&env));
            let cur = balances.get(to.clone()).unwrap_or(0);
            balances.set(to.clone(), cur + amount);
            env.storage().instance().set(&key, &balances);
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            from.require_auth();
            let key = symbol_short!("bal");
            let mut balances: Map<Address, i128> = env
                .storage()
                .instance()
                .get(&key)
                .unwrap_or_else(|| Map::new(&env));
            let from_bal = balances.get(from.clone()).unwrap_or(0);
            if from_bal < amount {
                panic!("insufficient balance");
            }
            balances.set(from.clone(), from_bal - amount);
            let to_bal = balances.get(to.clone()).unwrap_or(0);
            balances.set(to.clone(), to_bal + amount);
            env.storage().instance().set(&key, &balances);
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            let key = symbol_short!("bal");
            env.storage()
                .instance()
                .get::<_, Map<Address, i128>>(&key)
                .unwrap_or_else(|| Map::new(&env))
                .get(id)
                .unwrap_or(0)
        }
    }
}

use mock_token::{MockToken, MockTokenClient};

// ─── Test context ───────────────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    admin: Address,
    user: Address,
    attacker: Address,
    royalty_recipient: Address,
    token: Address,
    token_id: TokenId,
    nft: ClipCashNFTClient<'static>,
    token_client: MockTokenClient<'static>,
}

const LISTING_PRICE: i128 = 1_000_000;

fn setup(platform_fee_bps: u32) -> Ctx {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let attacker = Address::generate(&env);
    let royalty_recipient = Address::generate(&env);

    let token_addr = env.register(MockToken, ());
    let token_client = MockTokenClient::new(&env, &token_addr);

    let contract_id = env.register(ClipCashNFT, ());
    let nft = ClipCashNFTClient::new(&env, &contract_id);

    env.mock_all_auths();
    nft.init(&admin);

    let params = MintParams {
        owner: user.clone(),
        clip_id: 0,
        metadata_uri: String::from_str(&env, "ipfs://clip/1"),
        royalty: Royalty {
            recipients: soroban_sdk::vec![
                &env,
                crate::types::RoyaltyRecipient {
                    recipient: royalty_recipient.clone(),
                    basis_points: 500,
                },
            ],
            asset_address: Some(token_addr.clone()),
        },
        signature_hash: BytesN::from_array(&env, &[0u8; 32]),
        creator_address: Some(user.clone()),
        creator_display_name: None,
    };
    let token_id = nft.mint(&params);

    let config = Config {
        admin: admin.clone(),
        max_royalty_bps: 1_000,
        mint_cooldown_secs: 0,
        platform_fee_bps,
    };
    nft.set_config(&admin, &config);

    // Fund buyer (user) so purchases can settle.
    token_client.mint(&user, &100_000_000);

    Ctx {
        env,
        admin,
        user,
        attacker,
        royalty_recipient,
        token: token_addr,
        token_id,
        nft,
        token_client,
    }
}

// ─── 1. Unauthorized callers ───────────────────────────────────────────────────
//
// Verify that every admin-gated function rejects a caller that is not the
// contract administrator.

#[test]
fn set_default_royalty_bps_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx.nft.try_set_default_royalty_bps(&ctx.attacker, &500);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to set default royalty bps"
    );
}

#[test]
fn pause_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx.nft.try_pause(&ctx.attacker, None);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to pause the contract"
    );
}

#[test]
fn unpause_rejects_non_admin() {
    let ctx = setup(0);
    // First pause as admin.
    ctx.nft.pause(&ctx.admin, None).unwrap();
    // Attacker tries to unpause.
    let result = ctx.nft.try_unpause(&ctx.attacker);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to unpause the contract"
    );
}

#[test]
fn add_currency_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx.nft.try_add_currency(&ctx.attacker, &ctx.token);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to add supported currencies"
    );
}

#[test]
fn remove_currency_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_remove_currency(&ctx.attacker, &ctx.token);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to remove supported currencies"
    );
}

#[test]
fn set_royalty_rejects_non_admin() {
    let ctx = setup(0);
    let royalty = Royalty {
        recipients: soroban_sdk::vec![
            &ctx.env,
            crate::types::RoyaltyRecipient {
                recipient: ctx.royalty_recipient.clone(),
                basis_points: 250,
            },
        ],
        asset_address: Some(ctx.token.clone()),
    };
    let result = ctx.nft.try_set_royalty(&ctx.attacker, &ctx.token_id, &royalty);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to set royalty"
    );
}

#[test]
fn freeze_token_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx.nft.try_freeze_token(&ctx.attacker, &ctx.token_id, None);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to freeze tokens"
    );
}

#[test]
fn unfreeze_token_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx.nft.try_unfreeze_token(&ctx.attacker, &ctx.token_id);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to unfreeze tokens"
    );
}

#[test]
fn reassign_creator_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_reassign_creator(&ctx.attacker, &ctx.token_id, &ctx.attacker);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to reassign creators"
    );
}

#[test]
fn set_royalty_payments_disabled_rejects_non_admin() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_set_royalty_payments_disabled(&ctx.attacker, &true);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to toggle royalty payments"
    );
}

// ─── 2. Invalid token states ───────────────────────────────────────────────────
//
// Verify that operations on non-existent tokens fail with the expected error
// rather than silently succeeding or corrupting state.

#[test]
fn set_royalty_on_nonexistent_token_rejected() {
    let ctx = setup(0);
    let fake_token_id: TokenId = 99_999;
    let royalty = Royalty {
        recipients: soroban_sdk::vec![
            &ctx.env,
            crate::types::RoyaltyRecipient {
                recipient: ctx.royalty_recipient.clone(),
                basis_points: 250,
            },
        ],
        asset_address: Some(ctx.token.clone()),
    };
    let result = ctx.nft.try_set_royalty(&ctx.admin, &fake_token_id, &royalty);
    assert!(
        result.is_err(),
        "setting royalty on a non-existent token must fail"
    );
}

#[test]
fn freeze_nonexistent_token_rejected() {
    let ctx = setup(0);
    let result = ctx.nft.try_freeze_token(&ctx.admin, &99_999, None);
    assert!(
        result.is_err(),
        "freezing a non-existent token must fail"
    );
}

#[test]
fn unfreeze_nonexistent_token_rejected() {
    let ctx = setup(0);
    let result = ctx.nft.try_unfreeze_token(&ctx.admin, &99_999);
    assert!(
        result.is_err(),
        "unfreezing a non-existent token must fail"
    );
}

#[test]
fn reassign_creator_on_nonexistent_token_rejected() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_reassign_creator(&ctx.admin, &99_999, &ctx.user);
    assert!(
        result.is_err(),
        "reassigning creator on a non-existent token must fail"
    );
}

#[test]
fn buy_nonexistent_listing_rejected() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_buy_listing(&ctx.user, &99_999, &ctx.token, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "buying a non-existent listing must fail"
    );
}

#[test]
fn cancel_nonexistent_listing_rejected() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_cancel_listing(&ctx.user, &99_999);
    assert!(
        result.is_err(),
        "cancelling a non-existent listing must fail"
    );
}

#[test]
fn get_royalty_on_nonexistent_token_rejected() {
    let ctx = setup(0);
    let result = ctx.nft.try_get_royalty(&99_999);
    assert!(
        result.is_err(),
        "reading royalty on a non-existent token must fail"
    );
}

// ─── 3. Paused contract operations ─────────────────────────────────────────────
//
// Verify that all state-changing functions are blocked when the contract is
// paused.

#[test]
fn update_listing_rejected_when_paused() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    // Pause the contract.
    ctx.nft.pause(&ctx.admin, None).unwrap();

    let result = ctx
        .nft
        .update_listing(&ctx.user, &ctx.token_id, &2_000_000, &0);
    assert!(
        result.is_err(),
        "update_listing must be rejected when paused"
    );
}

#[test]
fn buy_listing_rejected_when_paused() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    ctx.nft.pause(&ctx.admin, None).unwrap();

    let result = ctx.nft.try_buy_listing(&ctx.user, &ctx.token_id, &ctx.token, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "buy_listing must be rejected when paused"
    );
}

#[test]
fn make_offer_rejected_when_paused() {
    let ctx = setup(0);
    ctx.nft.pause(&ctx.admin, None).unwrap();

    let result = ctx
        .nft
        .try_make_offer(&ctx.user, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0);
    assert!(
        result.is_err(),
        "make_offer must be rejected when paused"
    );
}

#[test]
fn accept_offer_rejected_when_paused() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();
    ctx.nft.pause(&ctx.admin, None).unwrap();

    let result = ctx.nft.try_accept_offer(&ctx.user, &ctx.token_id);
    assert!(
        result.is_err(),
        "accept_offer must be rejected when paused"
    );
}

#[test]
fn pause_rejected_when_already_paused() {
    let ctx = setup(0);
    ctx.nft.pause(&ctx.admin, None).unwrap();
    let result = ctx.nft.try_pause(&ctx.admin, None);
    assert!(
        result.is_err(),
        "pause must be rejected when already paused"
    );
}

#[test]
fn unpause_rejected_when_not_paused() {
    let ctx = setup(0);
    let result = ctx.nft.try_unpause(&ctx.admin);
    assert!(
        result.is_err(),
        "unpause must be rejected when not paused"
    );
}

// ─── 4. Unauthorized administrative operations ─────────────────────────────────
//
// Verify mint authorization, config validation, and royalty admin checks.

#[test]
fn mint_rejects_unauthorized_minter() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(AtomicMintContract, ());
    let client = AtomicMintContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    client.init(&admin);

    let attacker = Address::generate(&env);
    let params = MintParams {
        owner: attacker.clone(),
        clip_id: 1,
        metadata_uri: String::from_str(&env, "ipfs://clip/attacker"),
        royalty: Royalty {
            recipients: soroban_sdk::vec![
                &env,
                crate::types::RoyaltyRecipient {
                    recipient: attacker.clone(),
                    basis_points: 500,
                },
            ],
            asset_address: None,
        },
        signature_hash: BytesN::from_array(&env, &[0u8; 32]),
        creator_address: None,
        creator_display_name: None,
    };

    let result = client.try_mint(&params);
    assert!(
        result.is_err(),
        "unauthorized caller must not be able to mint"
    );
}

#[test]
fn mint_rejects_when_not_initialized() {
    let env = Env::default();
    let contract_id = env.register(AtomicMintContract, ());
    let client = AtomicMintContractClient::new(&env, &contract_id);
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let params = MintParams {
        owner: admin.clone(),
        clip_id: 1,
        metadata_uri: String::from_str(&env, "ipfs://clip/1"),
        royalty: Royalty {
            recipients: soroban_sdk::vec![
                &env,
                crate::types::RoyaltyRecipient {
                    recipient: admin.clone(),
                    basis_points: 500,
                },
            ],
            asset_address: None,
        },
        signature_hash: BytesN::from_array(&env, &[0u8; 32]),
        creator_address: None,
        creator_display_name: None,
    };

    let result = client.try_mint(&params);
    assert!(
        result.is_err(),
        "mint must fail when the contract is not initialized"
    );
}

#[test]
fn set_config_rejects_invalid_basis_points() {
    let ctx = setup(0);
    let bad = Config {
        admin: ctx.admin.clone(),
        max_royalty_bps: 20_000,
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    };
    let result = ctx.nft.try_set_config(&ctx.admin, &bad);
    assert!(
        result.is_err(),
        "config with invalid royalty bps must be rejected"
    );
}

#[test]
fn set_config_rejects_invalid_platform_fee() {
    let ctx = setup(0);
    let bad = Config {
        admin: ctx.admin.clone(),
        max_royalty_bps: 500,
        mint_cooldown_secs: 0,
        platform_fee_bps: 20_000,
    };
    let result = ctx.nft.try_set_config(&ctx.admin, &bad);
    assert!(
        result.is_err(),
        "config with invalid platform fee must be rejected"
    );
}

#[test]
fn set_config_rejects_unauthorized_caller() {
    let ctx = setup(0);
    let cfg = Config {
        admin: ctx.user.clone(),
        max_royalty_bps: 500,
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    };
    let result = ctx.nft.try_set_config(&ctx.attacker, &cfg);
    assert!(
        result.is_err(),
        "non-admin must not be allowed to call set_config"
    );
}

// ─── 5. Guard presence verification ────────────────────────────────────────────
//
// Verify that each protected function actually executes at least one guard
// before mutating state. These tests assert that the guard is *triggered*
// (not just present in the source) by observing the error returned when the
// guard condition is violated.

#[test]
fn revoke_approval_rejects_non_owner() {
    let ctx = setup(0);
    // Attacker tries to revoke approval on a token they don't own.
    let result = ctx.nft.try_revoke_approval(&ctx.attacker, &ctx.token_id);
    assert!(
        result.is_err(),
        "revoke_approval must verify token ownership"
    );
}

#[test]
fn revoke_operator_approval_rejects_non_owner() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_revoke_operator_approval(&ctx.attacker, &ctx.user);
    assert!(
        result.is_err(),
        "revoke_operator_approval must require owner auth"
    );
}

#[test]
fn list_nft_rejects_non_owner() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.attacker.clone(),
    };
    let result = ctx.nft.create_listing(&req);
    assert!(
        result.is_err(),
        "list_nft must reject a seller who doesn't own the token"
    );
}

#[test]
fn cancel_listing_rejects_non_owner() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    let result = ctx.nft.try_cancel_listing(&ctx.attacker, &ctx.token_id);
    assert!(
        result.is_err(),
        "cancel_listing must reject a caller who didn't create the listing"
    );
}

#[test]
fn update_listing_rejects_non_seller() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    let result = ctx
        .nft
        .try_update_listing(&ctx.attacker, &ctx.token_id, &2_000_000, &0);
    assert!(
        result.is_err(),
        "update_listing must reject a caller who didn't create the listing"
    );
}

#[test]
fn buy_listing_rejects_seller_as_buyer() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    // Seller cannot buy their own listing (self-transfer guard).
    let result = ctx
        .nft
        .try_buy_listing(&ctx.user, &ctx.token_id, &ctx.token, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "buy_listing must reject the seller as buyer"
    );
}

#[test]
fn buy_listing_rejects_wrong_payment_asset() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    let wrong_asset = Address::generate(&ctx.env);
    let result = ctx
        .nft
        .try_buy_listing(&ctx.user, &ctx.token_id, &wrong_asset, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "buy_listing must reject a mismatched payment asset"
    );
}

#[test]
fn buy_listing_rejects_wrong_amount() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    let result = ctx
        .nft
        .try_buy_listing(&ctx.user, &ctx.token_id, &ctx.token, &999_999);
    assert!(
        result.is_err(),
        "buy_listing must reject an incorrect payment amount"
    );
}

#[test]
fn make_offer_rejects_negative_price() {
    let ctx = setup(0);
    let result = ctx
        .nft
        .try_make_offer(&ctx.user, &ctx.token_id, &(-100), &ctx.token, &0);
    assert!(
        result.is_err(),
        "make_offer must reject a negative price"
    );
}

#[test]
fn accept_offer_rejects_non_owner() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.user, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();

    // Attacker (not the token owner) tries to accept the offer.
    let result = ctx.nft.try_accept_offer(&ctx.attacker, &ctx.token_id);
    assert!(
        result.is_err(),
        "accept_offer must verify that the caller is the token owner"
    );
}

#[test]
fn cancel_offer_rejects_non_buyer() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.user, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();

    let result = ctx.nft.try_cancel_offer(&ctx.attacker, &ctx.token_id);
    assert!(
        result.is_err(),
        "cancel_offer must verify that the caller is the buyer or an operator"
    );
}

#[test]
fn royalty_info_rejects_nonexistent_token() {
    let ctx = setup(0);
    let result = ctx.nft.try_royalty_info(&99_999, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "royalty_info must reject a non-existent token"
    );
}

#[test]
fn get_cumulative_earnings_does_not_panic_for_nonexistent_token() {
    let ctx = setup(0);
    // Read-only function should handle missing token gracefully.
    let earnings = ctx.nft.get_cumulative_earnings(&99_999);
    assert!(
        earnings == 0,
        "cumulative earnings for a non-existent token should be 0"
    );
}

// ─── 6. Admin-only royalty admin guard ─────────────────────────────────────────

#[test]
fn royalty_admin_guard_rejects_non_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(ClipCashNFT, ());
    let nft = ClipCashNFTClient::new(&env, &contract_id);
    env.mock_all_auths();
    nft.init(&admin);

    let attacker = Address::generate(&env);
    let result = nft.try_set_royalty_payments_disabled(&attacker, &true);
    assert!(
        result.is_err(),
        "only the contract admin may toggle royalty payments"
    );
}

#[test]
fn admin_can_pause_and_unpause() {
    let ctx = setup(0);
    // Admin should be able to pause and then unpause.
    ctx.nft.pause(&ctx.admin, None).unwrap();
    assert!(ctx.nft.is_paused());
    ctx.nft.unpause(&ctx.admin).unwrap();
    assert!(!ctx.nft.is_paused());
}

#[test]
fn admin_can_freeze_and_unfreeze_token() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();
    ctx.nft.unfreeze_token(&ctx.admin, &ctx.token_id).unwrap();
}

// ─── 7. Approval revocation authorization (Issue: Prevent Unauthorized Approval Revocation) ──

/// A non-owner cannot revoke a single-token approval that does not belong to them.
#[test]
fn revoke_approval_rejects_attacker_for_others_token() {
    let ctx = setup(0);
    // Grant an approval first (owner approves the attacker's address irrelevant
    // to the revocation attempt — we want to make sure the attacker cannot
    // revoke on a token they don't own).
    let operator = Address::generate(&ctx.env);
    ctx.nft.approve(&ctx.user, &operator, &ctx.token_id).unwrap();

    // Attacker tries to revoke an approval on a token owned by `user`.
    let result = ctx.nft.try_revoke_approval(&ctx.attacker, &ctx.token_id);
    assert!(
        result.is_err(),
        "revoke_approval must reject callers who do not own the token"
    );

    // Verify the approval is still intact after the failed attempt.
    let still_approved = ctx.nft.get_approved(&ctx.token_id);
    assert_eq!(
        still_approved,
        Some(operator),
        "approval must be preserved after an unauthorized revocation attempt"
    );
}

/// The token owner can successfully revoke their own token's approval.
#[test]
fn revoke_approval_succeeds_for_token_owner() {
    let ctx = setup(0);
    let operator = Address::generate(&ctx.env);
    ctx.nft.approve(&ctx.user, &operator, &ctx.token_id).unwrap();

    // Owner revokes the approval — must succeed.
    ctx.nft.revoke_approval(&ctx.user, &ctx.token_id).unwrap();

    // Approval must now be gone.
    let approval = ctx.nft.get_approved(&ctx.token_id);
    assert_eq!(
        approval, None,
        "approval must be cleared after a successful owner revocation"
    );
}

/// `revoke_approval` is blocked while the contract is paused, even for the
/// token owner.
#[test]
fn revoke_approval_blocked_when_paused() {
    let ctx = setup(0);
    let operator = Address::generate(&ctx.env);
    ctx.nft.approve(&ctx.user, &operator, &ctx.token_id).unwrap();

    ctx.nft.pause(&ctx.admin, None).unwrap();

    let result = ctx.nft.try_revoke_approval(&ctx.user, &ctx.token_id);
    assert_eq!(
        result,
        Err(Ok(crate::types::Error::ContractPaused)),
        "revoke_approval must return ContractPaused when the contract is paused"
    );

    // Approval must be unchanged while paused.
    ctx.nft.unpause(&ctx.admin).unwrap();
    let still_approved = ctx.nft.get_approved(&ctx.token_id);
    assert_eq!(
        still_approved,
        Some(operator),
        "approval must be preserved while the contract is paused"
    );
}

/// A third party cannot revoke an operator-for-all approval granted by another
/// owner.
#[test]
fn revoke_operator_approval_rejects_non_owner_caller() {
    let ctx = setup(0);
    let operator = Address::generate(&ctx.env);
    ctx.nft
        .set_approval_for_all(&ctx.user, &operator, &true)
        .unwrap();

    // Attacker attempts to revoke the operator approval for a different owner.
    let result = ctx
        .nft
        .try_revoke_operator_approval(&ctx.attacker, &ctx.user);
    assert!(
        result.is_err(),
        "revoke_operator_approval must reject callers who are not the owner"
    );

    // Operator approval must still be valid.
    assert!(
        ctx.nft.is_approved_for_all(&ctx.user, &operator),
        "operator approval must be preserved after an unauthorized revocation attempt"
    );
}

/// The owner can revoke their own operator-for-all approval.
#[test]
fn revoke_operator_approval_succeeds_for_owner() {
    let ctx = setup(0);
    let operator = Address::generate(&ctx.env);
    ctx.nft
        .set_approval_for_all(&ctx.user, &operator, &true)
        .unwrap();

    ctx.nft
        .revoke_operator_approval(&ctx.user, &operator)
        .unwrap();

    assert!(
        !ctx.nft.is_approved_for_all(&ctx.user, &operator),
        "operator approval must be cleared after a successful owner revocation"
    );
}

/// `revoke_operator_approval` is blocked while the contract is paused, even
/// for the owner.
#[test]
fn revoke_operator_approval_blocked_when_paused() {
    let ctx = setup(0);
    let operator = Address::generate(&ctx.env);
    ctx.nft
        .set_approval_for_all(&ctx.user, &operator, &true)
        .unwrap();

    ctx.nft.pause(&ctx.admin, None).unwrap();

    let result = ctx
        .nft
        .try_revoke_operator_approval(&ctx.user, &operator);
    assert_eq!(
        result,
        Err(Ok(crate::types::Error::ContractPaused)),
        "revoke_operator_approval must return ContractPaused when the contract is paused"
    );
}

/// `approve` is blocked while the contract is paused.
#[test]
fn approve_blocked_when_paused() {
    let ctx = setup(0);
    ctx.nft.pause(&ctx.admin, None).unwrap();

    let operator = Address::generate(&ctx.env);
    let result = ctx.nft.try_approve(&ctx.user, &operator, &ctx.token_id);
    assert_eq!(
        result,
        Err(Ok(crate::types::Error::ContractPaused)),
        "approve must return ContractPaused when the contract is paused"
    );
}

/// `set_approval_for_all` is blocked while the contract is paused.
#[test]
fn set_approval_for_all_blocked_when_paused() {
    let ctx = setup(0);
    ctx.nft.pause(&ctx.admin, None).unwrap();

    let operator = Address::generate(&ctx.env);
    let result = ctx
        .nft
        .try_set_approval_for_all(&ctx.user, &operator, &true);
    assert_eq!(
        result,
        Err(Ok(crate::types::Error::ContractPaused)),
        "set_approval_for_all must return ContractPaused when the contract is paused"
    );
}

// ─── 8. Token frozen guard (Issue: Create Token Frozen Guard) ──────────────────
//
// Verify that the freeze-state guard prevents operations against frozen NFTs,
// allows operations on unfrozen tokens, and permits administrative operations
// that must remain available while a token is frozen.

/// The freeze state of a freshly minted token is `false`.
#[test]
fn freeze_state_defaults_to_unfrozen() {
    let ctx = setup(0);
    assert!(
        !ctx.nft.is_token_frozen(&ctx.token_id),
        "a freshly minted token must not be frozen"
    );
}

/// Freezing a token flips its freeze state, and unfreezing restores it.
#[test]
fn freeze_state_toggles_with_freeze_and_unfreeze() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();
    assert!(
        ctx.nft.is_token_frozen(&ctx.token_id),
        "token must report frozen after freeze_token"
    );

    ctx.nft.unfreeze_token(&ctx.admin, &ctx.token_id).unwrap();
    assert!(
        !ctx.nft.is_token_frozen(&ctx.token_id),
        "token must report unfrozen after unfreeze_token"
    );
}

/// A frozen token cannot be listed for sale.
#[test]
fn create_listing_rejected_when_token_frozen() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    let result = ctx.nft.create_listing(&req);
    assert!(
        result.is_err(),
        "create_listing must be rejected when the token is frozen"
    );
}

/// A frozen token cannot be transferred via `transfer`.
#[test]
fn transfer_rejected_when_token_frozen() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let recipient = Address::generate(&ctx.env);
    let result = ctx.nft.try_transfer(&ctx.user, &recipient, &ctx.token_id);
    assert!(
        result.is_err(),
        "transfer must be rejected when the token is frozen"
    );

    // Ownership must be unchanged after the rejected transfer.
    assert_eq!(
        ctx.nft.owner_of(&ctx.token_id),
        ctx.user,
        "ownership must be preserved after a rejected transfer of a frozen token"
    );
}

/// A frozen token cannot be transferred via `transfer_from`.
#[test]
fn transfer_from_rejected_when_token_frozen() {
    let ctx = setup(0);
    let operator = Address::generate(&ctx.env);
    ctx.nft.approve(&ctx.user, &operator, &ctx.token_id).unwrap();
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let recipient = Address::generate(&ctx.env);
    let result = ctx
        .nft
        .try_transfer_from(&operator, &ctx.user, &recipient, &ctx.token_id);
    assert!(
        result.is_err(),
        "transfer_from must be rejected when the token is frozen"
    );
}

/// A frozen token cannot be burned.
#[test]
fn burn_rejected_when_token_frozen() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let result = ctx.nft.try_burn(&ctx.user, &ctx.token_id);
    assert!(
        result.is_err(),
        "burn must be rejected when the token is frozen"
    );

    // Token must still exist after the rejected burn.
    assert_eq!(
        ctx.nft.owner_of(&ctx.token_id),
        ctx.user,
        "token must still be owned after a rejected burn of a frozen token"
    );
}

/// A frozen token cannot be sold through an existing listing.
#[test]
fn buy_listing_rejected_when_token_frozen() {
    let ctx = setup(0);
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.user.clone(),
    };
    ctx.nft.create_listing(&req).unwrap();

    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let buyer = Address::generate(&ctx.env);
    ctx.token_client.mint(&buyer, &100_000_000);
    let result = ctx
        .nft
        .try_buy_listing(&buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "buy_listing must be rejected when the token is frozen"
    );
}

/// A frozen token cannot be offered for sale via `make_offer`.
#[test]
fn make_offer_rejected_when_token_frozen() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let result = ctx
        .nft
        .try_make_offer(&ctx.user, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0);
    assert!(
        result.is_err(),
        "make_offer must be rejected when the token is frozen"
    );
}

/// A frozen token cannot have an offer accepted.
#[test]
fn accept_offer_rejected_when_token_frozen() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.user, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();

    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    let result = ctx.nft.try_accept_offer(&ctx.user, &ctx.token_id);
    assert!(
        result.is_err(),
        "accept_offer must be rejected when the token is frozen"
    );
}

/// Operations on an unfrozen token are allowed (guard does not over-block).
#[test]
fn transfer_allowed_when_token_unfrozen() {
    let ctx = setup(0);
    let recipient = Address::generate(&ctx.env);
    ctx.nft.transfer(&ctx.user, &recipient, &ctx.token_id).unwrap();
    assert_eq!(
        ctx.nft.owner_of(&ctx.token_id),
        recipient,
        "transfer must succeed when the token is not frozen"
    );
}

/// Administrative operations remain available while a token is frozen.
#[test]
fn admin_operations_allowed_when_token_frozen() {
    let ctx = setup(0);
    ctx.nft.freeze_token(&ctx.admin, &ctx.token_id, None).unwrap();

    // Admin can update the token's royalty while it is frozen.
    let royalty = Royalty {
        recipients: soroban_sdk::vec![
            &ctx.env,
            crate::types::RoyaltyRecipient {
                recipient: ctx.royalty_recipient.clone(),
                basis_points: 250,
            },
        ],
        asset_address: Some(ctx.token.clone()),
    };
    ctx.nft
        .set_royalty(&ctx.admin, &ctx.token_id, &royalty)
        .unwrap();

    // Admin can reassign the creator while the token is frozen.
    let new_creator = Address::generate(&ctx.env);
    ctx.nft
        .reassign_creator(&ctx.admin, &ctx.token_id, &new_creator)
        .unwrap();

    // Admin can unfreeze the token.
    ctx.nft.unfreeze_token(&ctx.admin, &ctx.token_id).unwrap();
    assert!(
        !ctx.nft.is_token_frozen(&ctx.token_id),
        "admin must be able to unfreeze a frozen token"
    );
}

/// A non-admin cannot freeze a token, so the guard cannot be bypassed by an
/// untrusted caller.
#[test]
fn non_admin_cannot_freeze_token() {
    let ctx = setup(0);
    let result = ctx.nft.try_freeze_token(&ctx.attacker, &ctx.token_id, None);
    assert!(
        result.is_err(),
        "non-admin must not be able to freeze a token"
    );
    assert!(
        !ctx.nft.is_token_frozen(&ctx.token_id),
        "token must remain unfrozen after a rejected freeze attempt"
    );
}
