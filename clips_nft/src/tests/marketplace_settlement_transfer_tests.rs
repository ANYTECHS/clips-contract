//! Marketplace settlement → NFT transfer integration tests.
//!
//! Verifies that `buy_listing` transfers NFT ownership **only after** the
//! marketplace payment settlement succeeds, and that every ownership index is
//! updated to reflect the new owner:
//!
//! * the dedicated owner record (`DataKey::TokenOwner`),
//! * the legacy `TokenData` entry (`DataKey::Token`),
//! * the owner-portfolio index (`DataKey::OwnerTokens`),
//! * the wallet token index (`DataKey::WalletTokens`).
//!
//! It also verifies that a failed payment (insufficient balance) aborts the
//! purchase atomically, leaving ownership and indexes untouched.

#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::Address as _,
    Address, BytesN, Env, Map, String,
};

use crate::{
    atomic_mint::MintParams,
    types::{Config, DataKey, Royalty, RoyaltyRecipient, TokenData, TokenId},
    ClipCashNFT, ClipCashNFTClient,
};

// ─── Mock SEP-41 token (mirrors guard_security_tests) ───────────────────────────

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

// ─── Test context ──────────────────────────────────────────────────────────────

const LISTING_PRICE: i128 = 1_000_000;

struct Ctx {
    env: Env,
    admin: Address,
    seller: Address,
    buyer: Address,
    token: Address,
    token_id: TokenId,
    nft: ClipCashNFTClient<'static>,
    token_client: MockTokenClient<'static>,
    contract_id: Address,
}

fn setup(platform_fee_bps: u32) -> Ctx {
    let env = Env::default();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let token_addr = env.register(MockToken, ());
    let token_client = MockTokenClient::new(&env, &token_addr);

    let contract_id = env.register(ClipCashNFT, ());
    let nft = ClipCashNFTClient::new(&env, &contract_id);

    env.mock_all_auths();
    nft.init(&admin);

    // Mint the NFT to the seller. A zero royalty keeps settlement math simple:
    // platform fee + seller net must equal the listing price.
    let params = MintParams {
        owner: seller.clone(),
        clip_id: 0,
        metadata_uri: String::from_str(&env, "ipfs://clip/1"),
        royalty: Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: seller.clone(),
                    basis_points: 0,
                },
            ],
            asset_address: Some(token_addr.clone()),
        },
        signature_hash: BytesN::from_array(&env, &[0u8; 32]),
        creator_address: Some(seller.clone()),
        creator_display_name: None,
    };
    let token_id = nft.mint(&params);

    nft.set_config(
        &admin,
        &Config {
            admin: admin.clone(),
            max_royalty_bps: 1_000,
            mint_cooldown_secs: 0,
            platform_fee_bps,
        },
    );

    // Register the payment asset as supported.
    nft.add_currency(&admin, &token_addr);

    // Fund the buyer so settlement can complete.
    token_client.mint(&buyer, &100_000_000);

    // List the NFT for sale.
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id,
        price: LISTING_PRICE,
        payment_asset: token_addr.clone(),
        expiration: 0,
        seller: seller.clone(),
    };
    nft.create_listing(&req);

    Ctx {
        env,
        admin,
        seller,
        buyer,
        token: token_addr,
        token_id,
        nft,
        token_client,
        contract_id,
    }
}

// ─── Helpers to read ownership indexes from contract storage ────────────────────

fn stored_owner(env: &Env, cid: &Address, token_id: TokenId) -> Address {
    env.as_contract(cid, || {
        crate::token_owner_storage::get_owner(env, token_id).unwrap()
    })
}

fn stored_token_data_owner(env: &Env, cid: &Address, token_id: TokenId) -> Address {
    env.as_contract(cid, || {
        let data: TokenData = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .unwrap();
        data.owner
    })
}

fn wallet_tokens(env: &Env, cid: &Address, wallet: &Address) -> soroban_sdk::Vec<TokenId> {
    env.as_contract(cid, || crate::wallet_token_index::get_wallet_tokens(env, wallet))
}

fn owner_portfolio(env: &Env, cid: &Address, owner: &Address) -> soroban_sdk::Vec<TokenId> {
    env.as_contract(cid, || crate::owner_portfolio::get_owner_portfolio(env, owner))
}

// ─── 1. Settlement succeeds → NFT + indexes updated ────────────────────────────

#[test]
fn successful_settlement_transfers_nft_and_updates_indexes() {
    let ctx = setup(0);
    let cid = ctx.contract_id.clone();
    let token_id = ctx.token_id;

    // Pre-conditions: seller owns the token and is indexed.
    assert_eq!(stored_owner(&ctx.env, &cid, token_id), ctx.seller);
    assert_eq!(
        stored_token_data_owner(&ctx.env, &cid, token_id),
        ctx.seller
    );
    assert!(wallet_tokens(&ctx.env, &cid, &ctx.seller).contains(&token_id));

    // Execute the purchase.
    ctx.nft
        .buy_listing(&ctx.buyer, &token_id, &ctx.token, &LISTING_PRICE);

    // Ownership record moved to the buyer.
    assert_eq!(stored_owner(&ctx.env, &cid, token_id), ctx.buyer);
    // Legacy TokenData entry synced to the buyer.
    assert_eq!(
        stored_token_data_owner(&ctx.env, &cid, token_id),
        ctx.buyer
    );
    // Wallet token index moved from seller to buyer.
    assert!(!wallet_tokens(&ctx.env, &cid, &ctx.seller).contains(&token_id));
    assert!(wallet_tokens(&ctx.env, &cid, &ctx.buyer).contains(&token_id));
}

#[test]
fn successful_settlement_moves_owner_portfolio_index() {
    let ctx = setup(0);
    let cid = ctx.contract_id.clone();
    let token_id = ctx.token_id;

    // Mint does not populate the owner portfolio, so seed it to exercise the
    // move path in `buy_listing`.
    ctx.env.as_contract(&cid, || {
        crate::owner_portfolio::add_token_to_owner(&ctx.env, &ctx.seller, token_id).unwrap();
    });
    assert!(owner_portfolio(&ctx.env, &cid, &ctx.seller).contains(&token_id));

    ctx.nft
        .buy_listing(&ctx.buyer, &token_id, &ctx.token, &LISTING_PRICE);

    assert!(!owner_portfolio(&ctx.env, &cid, &ctx.seller).contains(&token_id));
    assert!(owner_portfolio(&ctx.env, &cid, &ctx.buyer).contains(&token_id));
}

#[test]
fn successful_settlement_marks_listing_sold() {
    let ctx = setup(0);
    let token_id = ctx.token_id;

    ctx.nft
        .buy_listing(&ctx.buyer, &token_id, &ctx.token, &LISTING_PRICE);

    // A second purchase of the same listing must be rejected because the
    // listing is no longer purchasable.
    let result = ctx
        .nft
        .try_buy_listing(&ctx.buyer, &token_id, &ctx.token, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "a sold listing must not be purchasable again"
    );
}

// ─── 2. Payment fails → transfer is prevented (atomicity) ───────────────────────

#[test]
fn failed_payment_prevents_nft_transfer() {
    let ctx = setup(0);
    let cid = ctx.contract_id.clone();
    let token_id = ctx.token_id;

    // Drain the buyer's balance so the settlement transfer panics.
    let drain = ctx.token_client.balance(&ctx.buyer);
    ctx.env.as_contract(&ctx.contract_id, || {
        // No-op: balance lives in the mock token contract, drained below.
        let _ = drain;
    });
    // Transfer the buyer's entire balance out via the mock token directly.
    let sink = Address::generate(&ctx.env);
    ctx.token_client.transfer(&ctx.buyer, &sink, &drain);
    assert_eq!(ctx.token_client.balance(&ctx.buyer), 0);

    // The purchase must fail.
    let result = ctx
        .nft
        .try_buy_listing(&ctx.buyer, &token_id, &ctx.token, &LISTING_PRICE);
    assert!(
        result.is_err(),
        "buy_listing must fail when the buyer cannot pay"
    );

    // Ownership and indexes must be unchanged.
    assert_eq!(stored_owner(&ctx.env, &cid, token_id), ctx.seller);
    assert_eq!(
        stored_token_data_owner(&ctx.env, &cid, token_id),
        ctx.seller
    );
    assert!(wallet_tokens(&ctx.env, &cid, &ctx.seller).contains(&token_id));
    assert!(!wallet_tokens(&ctx.env, &cid, &ctx.buyer).contains(&token_id));
    assert!(owner_portfolio(&ctx.env, &cid, &ctx.seller).contains(&token_id));
    assert!(!owner_portfolio(&ctx.env, &cid, &ctx.buyer).contains(&token_id));
}