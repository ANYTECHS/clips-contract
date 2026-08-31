//! Marketplace royalty integration test suite.
//!
//! Comprehensive integration tests that verify the royalty calculation module
//! is properly integrated into marketplace sales (listings and offers).
//!
//! Acceptance criteria:
//! 1. Retrieve NFT royalty configuration
//! 2. Calculate royalty
//! 3. Include royalty in settlement
//! 4. Add integration tests

#![cfg(test)]

use soroban_sdk::{contract, contractimpl, symbol_short, testutils::Address as _, Address, BytesN, String};
use crate::{
    atomic_mint::MintParams,
    listing_request::ListingRequest,
    types::{Config, Royalty, RoyaltyRecipient, TokenId},
    ClipCashNFT, ClipCashNFTClient,
};

// ─── Mock SEP-41 token ────────────────────────────────────────────────────────

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let key = symbol_short!("bal");
        let mut balances: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let cur = balances.get(to.clone()).unwrap_or(0);
        balances.set(to.clone(), cur + amount);
        env.storage().instance().set(&key, &balances);
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let key = symbol_short!("bal");
        let mut balances: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
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
            .get::<_, soroban_sdk::Map<Address, i128>>(&key)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env))
            .get(id)
            .unwrap_or(0)
    }
}

// ─── Test context ───────────────────────────────────────────────────────────

struct TestContext {
    env: Env,
    admin: Address,
    seller: Address,
    buyer: Address,
    royalty_recipient: Address,
    token: Address,
    token_id: TokenId,
    nft: ClipCashNFTClient,
    token_client: MockTokenClient,
}

const SALE_PRICE: i128 = 1_000_000;

fn setup_with_royalty(royalty_bps: u32, platform_fee_bps: u32) -> TestContext {
    let env = Env::default();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let royalty_recipient = Address::generate(&env);

    let token_id_addr = env.register(MockToken, ());
    let token_client = MockTokenClient::new(&env, &token_id_addr);

    let contract_id = env.register(ClipCashNFT, ());
    let nft = ClipCashNFTClient::new(&env, &contract_id);

    env.mock_all_auths();

    nft.init(&admin);

    // Create NFT with royalty configuration
    let mut recipients = soroban_sdk::Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: royalty_recipient.clone(),
        basis_points: royalty_bps,
    });
    let royalty = Royalty {
        recipients,
        asset_address: Some(token_id_addr.clone()),
    };
    let params = MintParams {
        owner: seller.clone(),
        clip_id: 0,
        metadata_uri: String::from_str(&env, "ipfs://clip/1"),
        royalty,
        signature_hash: BytesN::from_array(&env, &[0u8; 32]),
        creator_address: Some(seller.clone()),
        creator_display_name: None,
    };
    let token_id = nft.mint(&params);

    // Configure platform fee
    let config = Config {
        admin: admin.clone(),
        max_royalty_bps: 10_000,
        mint_cooldown_secs: 0,
        platform_fee_bps,
    };
    nft.set_config(&admin, &config);

    // Fund the buyer
    token_client.mint(&buyer, &100_000_000);

    TestContext {
        env,
        admin,
        seller,
        buyer,
        royalty_recipient,
        token: token_id_addr,
        token_id,
        nft,
        token_client,
    }
}

fn create_listing_request(ctx: &TestContext) -> ListingRequest {
    ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: SALE_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.seller.clone(),
    }
}

// ─── Integration tests ───────────────────────────────────────────────────────

#[test]
fn test_marketplace_retrieves_royalty_configuration() {
    // Acceptance criteria 1: Retrieve NFT royalty configuration
    let ctx = setup_with_royalty(500, 0); // 5% royalty

    // Verify royalty configuration is stored and retrievable
    let royalty = ctx.nft.get_royalty(&ctx.token_id).unwrap();
    assert_eq!(royalty.recipients.len(), 1);
    assert_eq!(royalty.recipients.get_unchecked(0).basis_points, 500);
    assert_eq!(
        royalty.recipients.get_unchecked(0).recipient,
        ctx.royalty_recipient
    );
}

#[test]
fn test_marketplace_calculates_royalty_correctly() {
    // Acceptance criteria 2: Calculate royalty
    let royalty_bps = 500; // 5%
    let ctx = setup_with_royalty(royalty_bps, 0);

    // Expected royalty: 1_000_000 * 500 / 10_000 = 50_000
    let expected_royalty = SALE_PRICE * royalty_bps as i128 / 10_000;

    ctx.nft.list_nft(&create_listing_request(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &SALE_PRICE)
        .unwrap();

    // Verify royalty was calculated and paid correctly
    assert_eq!(
        ctx.token_client.balance(&ctx.royalty_recipient),
        expected_royalty
    );
}

#[test]
fn test_marketplace_includes_royalty_in_settlement() {
    // Acceptance criteria 3: Include royalty in settlement
    let royalty_bps = 1000; // 10%
    let platform_fee_bps = 250; // 2.5%
    let ctx = setup_with_royalty(royalty_bps, platform_fee_bps);

    let expected_royalty = SALE_PRICE * royalty_bps as i128 / 10_000;
    let expected_platform_fee = SALE_PRICE * platform_fee_bps as i128 / 10_000;
    let expected_seller_net = SALE_PRICE - expected_royalty - expected_platform_fee;

    ctx.nft.list_nft(&create_listing_request(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &SALE_PRICE)
        .unwrap();

    // Verify all parties received correct amounts
    assert_eq!(
        ctx.token_client.balance(&ctx.royalty_recipient),
        expected_royalty
    );
    assert_eq!(
        ctx.token_client.balance(&ctx.nft.address),
        expected_platform_fee
    );
    assert_eq!(
        ctx.token_client.balance(&ctx.seller),
        expected_seller_net
    );

    // Verify total deductions don't exceed sale price
    assert!(expected_royalty + expected_platform_fee <= SALE_PRICE);
}

#[test]
fn test_offer_flow_includes_royalty_in_settlement() {
    // Verify royalty integration works for offer acceptance too
    let royalty_bps = 750; // 7.5%
    let ctx = setup_with_royalty(royalty_bps, 0);

    let expected_royalty = SALE_PRICE * royalty_bps as i128 / 10_000;
    let expected_seller_net = SALE_PRICE - expected_royalty;

    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &SALE_PRICE, &ctx.token, &0)
        .unwrap();
    ctx.nft.accept_offer(&ctx.seller, &ctx.token_id).unwrap();

    // Verify royalty was paid via offer flow
    assert_eq!(
        ctx.token_client.balance(&ctx.royalty_recipient),
        expected_royalty
    );
    assert_eq!(
        ctx.token_client.balance(&ctx.seller),
        expected_seller_net
    );
}

#[test]
fn test_zero_royalty_bypasses_payment() {
    // Verify zero royalty configuration is handled correctly
    let ctx = setup_with_royalty(0, 0); // 0% royalty

    ctx.nft.list_nft(&create_listing_request(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &SALE_PRICE)
        .unwrap();

    // Royalty recipient should receive nothing
    assert_eq!(ctx.token_client.balance(&ctx.royalty_recipient), 0);
    // Seller receives full amount
    assert_eq!(ctx.token_client.balance(&ctx.seller), SALE_PRICE);
}

#[test]
fn test_multi_recipient_royalty_distribution() {
    // Test royalty distribution across multiple recipients
    let env = Env::default();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let recipient1 = Address::generate(&env);
    let recipient2 = Address::generate(&env);

    let token_id_addr = env.register(MockToken, ());
    let token_client = MockTokenClient::new(&env, &token_id_addr);

    let contract_id = env.register(ClipCashNFT, ());
    let nft = ClipCashNFTClient::new(&env, &contract_id);

    env.mock_all_auths();
    nft.init(&admin);

    // Create NFT with multiple royalty recipients
    let mut recipients = soroban_sdk::Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: recipient1.clone(),
        basis_points: 300, // 3%
    });
    recipients.push_back(RoyaltyRecipient {
        recipient: recipient2.clone(),
        basis_points: 200, // 2%
    });
    let royalty = Royalty {
        recipients,
        asset_address: Some(token_id_addr.clone()),
    };
    let params = MintParams {
        owner: seller.clone(),
        clip_id: 0,
        metadata_uri: String::from_str(&env, "ipfs://clip/1"),
        royalty,
        signature_hash: BytesN::from_array(&env, &[0u8; 32]),
        creator_address: Some(seller.clone()),
        creator_display_name: None,
    };
    let token_id = nft.mint(&params);

    let config = Config {
        admin: admin.clone(),
        max_royalty_bps: 10_000,
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    };
    nft.set_config(&admin, &config);

    token_client.mint(&buyer, &100_000_000);

    let listing = ListingRequest {
        listing_id: 0,
        token_id,
        price: SALE_PRICE,
        payment_asset: token_id_addr.clone(),
        expiration: 0,
        seller: seller.clone(),
    };

    nft.list_nft(&listing).unwrap();
    nft.buy_listing(&buyer, &token_id, &token_id_addr, &SALE_PRICE)
        .unwrap();

    // Verify each recipient received their share
    let expected_recipient1 = SALE_PRICE * 300 / 10_000; // 30_000
    let expected_recipient2 = SALE_PRICE * 200 / 10_000; // 20_000
    let expected_seller = SALE_PRICE - expected_recipient1 - expected_recipient2;

    assert_eq!(token_client.balance(&recipient1), expected_recipient1);
    assert_eq!(token_client.balance(&recipient2), expected_recipient2);
    assert_eq!(token_client.balance(&seller), expected_seller);
}

#[test]
fn test_royalty_info_preview() {
    // Test royalty_info read-only preview function
    let ctx = setup_with_royalty(500, 0);

    let info = ctx.nft.royalty_info(&ctx.token_id, &SALE_PRICE).unwrap();
    assert_eq!(info.royalty_amount, SALE_PRICE * 500 / 10_000);
    assert_eq!(info.receiver, ctx.royalty_recipient);
}

#[test]
fn test_cumulative_earnings_tracking() {
    // Verify cumulative earnings are tracked after marketplace sale
    let ctx = setup_with_royalty(500, 0);

    let initial_earnings = ctx.nft.get_cumulative_earnings(&ctx.token_id);
    assert_eq!(initial_earnings, 0);

    ctx.nft.list_nft(&create_listing_request(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &SALE_PRICE)
        .unwrap();

    let expected_royalty = SALE_PRICE * 500 / 10_000;
    let final_earnings = ctx.nft.get_cumulative_earnings(&ctx.token_id);
    assert_eq!(final_earnings, expected_royalty);
}

#[test]
fn test_royalty_history_recorded() {
    // Verify royalty payment history is recorded
    let ctx = setup_with_royalty(500, 0);

    ctx.nft.list_nft(&create_listing_request(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &SALE_PRICE)
        .unwrap();

    let history = ctx.nft.get_royalty_history(&ctx.token_id);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get_unchecked(0).amount, SALE_PRICE * 500 / 10_000);
    assert_eq!(
        history.get_unchecked(0).recipient,
        ctx.royalty_recipient
    );
}
