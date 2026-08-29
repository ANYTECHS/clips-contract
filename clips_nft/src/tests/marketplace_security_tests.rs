//! Marketplace security test suite (issue #889).
//!
//! Covers the authorization and integrity guarantees of the listing, purchase,
//! and offer flows implemented for issues #871, #883, #884, #885, and #886.
//!
//! Every scenario below is exercised through the public [`ClipCashNFTClient`]
//! surface (i.e. exactly as an untrusted on-chain caller would invoke it).
//! Cryptographic caller authentication is enforced by `require_auth` (provided
//! by the Soroban host); these tests additionally assert the *business-logic*
//! authorization and invariant checks that the contract must enforce itself.

use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, Address, BytesN, Env, IntoVal,
    Map, String, Val,
};

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

// ─── Test context ───────────────────────────────────────────────────────────

struct Ctx {
    env: Env,
    admin: Address,
    seller: Address,
    buyer: Address,
    royalty_recipient: Address,
    attacker: Address,
    token: Address,
    token_id: TokenId,
    nft: ClipCashNFTClient,
    token_client: MockTokenClient,
}

const LISTING_PRICE: i128 = 1_000_000;

fn setup(platform_fee_bps: u32) -> Ctx {
    let env = Env::default();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let royalty_recipient = Address::generate(&env);
    let attacker = Address::generate(&env);

    let token_id_addr = env.register(MockToken, ());
    let token_client = MockTokenClient::new(&env, &token_id_addr);

    let contract_id = env.register(ClipCashNFT, ());
    let nft = ClipCashNFTClient::new(&env, &contract_id);

    env.mock_all_auths();

    nft.init(&admin);

    let mut recipients = soroban_sdk::Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: royalty_recipient.clone(),
        basis_points: 500,
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

    // Configure platform fee.
    let config = Config {
        admin: admin.clone(),
        max_royalty_bps: 1_000,
        mint_cooldown_secs: 0,
        platform_fee_bps,
    };
    nft.set_config(&admin, &config);

    // Fund the buyer so purchases/offers can settle.
    token_client.mint(&buyer, &100_000_000);

    Ctx {
        env,
        admin,
        seller,
        buyer,
        royalty_recipient,
        attacker,
        token: token_id_addr,
        token_id,
        nft,
        token_client,
    }
}

fn listing(ctx: &Ctx) -> ListingRequest {
    ListingRequest {
        listing_id: 0,
        token_id: ctx.token_id,
        price: LISTING_PRICE,
        payment_asset: ctx.token.clone(),
        expiration: 0,
        seller: ctx.seller.clone(),
    }
}

/// Assert that an event with the given short topic label was emitted.
fn emitted(env: &Env, label: &str) -> bool {
    let sym: Val = match label {
        "lst_can" => symbol_short!("lst_can").into_val(env),
        "lst_crt" => symbol_short!("lst_crt").into_val(env),
        "lst_upd" => symbol_short!("lst_upd").into_val(env),
        "nft_sold" => symbol_short!("nft_sold").into_val(env),
        "ofr_made" => symbol_short!("ofr_made").into_val(env),
        "ofr_acc" => symbol_short!("ofr_acc").into_val(env),
        "ofr_can" => symbol_short!("ofr_can").into_val(env),
        _ => return false,
    };
    env.events()
        .all()
        .iter()
        .any(|(_, topics, _)| topics.get(0) == sym)
}

// ─── Listing lifecycle ───────────────────────────────────────────────────────

#[test]
fn valid_listing_lifecycle() {
    let ctx = setup(0);
    let id = ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(id, 1);
    assert!(emitted(&ctx.env, "lst_crt"));

    let stored = ctx.nft.get_listing(&ctx.token_id).unwrap();
    assert_eq!(stored.seller, ctx.seller);
    assert_eq!(stored.price, LISTING_PRICE);

    // Update price + expiration.
    ctx.nft
        .update_listing(&ctx.seller, &ctx.token_id, &2_000_000, &1_800_000_000)
        .unwrap();
    assert!(emitted(&ctx.env, "lst_upd"));
    let updated = ctx.nft.get_listing(&ctx.token_id).unwrap();
    assert_eq!(updated.price, 2_000_000);
    assert_eq!(updated.expiration, 1_800_000_000);

    // Cancel.
    ctx.nft.cancel_listing(&ctx.seller, &ctx.token_id).unwrap();
    assert!(emitted(&ctx.env, "lst_can"));
    assert_eq!(
        ctx.nft.get_listing(&ctx.token_id),
        Err(crate::types::Error::ListingNotFound)
    );
}

#[test]
fn unauthorized_listing_by_non_owner_rejected() {
    let ctx = setup(0);
    // Attacker tries to list a token they do not own.
    let mut req = listing(&ctx);
    req.seller = ctx.attacker.clone();
    assert_eq!(
        ctx.nft.create_listing(&req),
        Err(crate::types::Error::Unauthorized)
    );
}

#[test]
fn duplicate_listing_rejected() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(
        ctx.nft.create_listing(&listing(&ctx)),
        Err(crate::types::Error::DuplicateListing)
    );
}

#[test]
fn update_requires_seller() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    // Attacker (not the seller) attempts to modify the listing.
    assert_eq!(
        ctx.nft.update_listing(&ctx.attacker, &ctx.token_id, &2_000_000, &0),
        Err(crate::types::Error::Unauthorized)
    );
}

#[test]
fn update_rejects_invalid_price() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(
        ctx.nft.update_listing(&ctx.seller, &ctx.token_id, &0, &0),
        Err(crate::types::Error::InvalidSalePrice)
    );
}

#[test]
fn update_rejects_expired_expiration() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(
        ctx.nft.update_listing(&ctx.seller, &ctx.token_id, &LISTING_PRICE, &100),
        Err(crate::types::Error::ListingExpired)
    );
}

#[test]
fn cancel_requires_seller() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(
        ctx.nft.cancel_listing(&ctx.attacker, &ctx.token_id),
        Err(crate::types::Error::Unauthorized)
    );
}

// ─── Purchase flow ───────────────────────────────────────────────────────────

#[test]
fn buy_transfers_nft_and_settles_funds() {
    let ctx = setup(100);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();

    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE)
        .unwrap();
    assert!(emitted(&ctx.env, "nft_sold"));

    // NFT ownership moved to buyer.
    assert_eq!(ctx.nft.owner_of(&ctx.token_id).unwrap(), ctx.buyer);

    // Royalty (5%) + platform fee (1%) deducted from the sale.
    let expected_royalty = LISTING_PRICE * 500 / 10_000;
    let expected_fee = LISTING_PRICE * 100 / 10_000;
    assert_eq!(ctx.token_client.balance(&ctx.royalty_recipient), expected_royalty);
    assert_eq!(ctx.token_client.balance(&ctx.nft.address), expected_fee);

    // Seller received the remainder.
    let seller_expected = LISTING_PRICE - expected_royalty - expected_fee;
    assert_eq!(ctx.token_client.balance(&ctx.seller), seller_expected);
}

#[test]
fn buy_unlisted_token_rejected() {
    let ctx = setup(0);
    assert_eq!(
        ctx.nft.buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE),
        Err(crate::types::Error::ListingNotFound)
    );
}

#[test]
fn buy_wrong_payment_asset_rejected() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    let other = ctx.env.register(MockToken, ());
    assert_eq!(
        ctx.nft.buy_listing(&ctx.buyer, &ctx.token_id, &other, &LISTING_PRICE),
        Err(crate::types::Error::PaymentAssetMismatch)
    );
}

#[test]
fn buy_wrong_amount_rejected() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(
        ctx.nft.buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &999_999),
        Err(crate::types::Error::IncorrectPaymentAmount)
    );
}

#[test]
fn buy_by_seller_rejected() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    assert_eq!(
        ctx.nft.buy_listing(&ctx.seller, &ctx.token_id, &ctx.token, &LISTING_PRICE),
        Err(crate::types::Error::SelfTransferNotAllowed)
    );
}

#[test]
fn buy_expired_listing_rejected() {
    let ctx = setup(0);
    let mut req = listing(&ctx);
    req.expiration = 100; // in the past relative to the default ledger time.
    ctx.nft.create_listing(&req).unwrap();
    assert_eq!(
        ctx.nft.buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE),
        Err(crate::types::Error::ListingExpired)
    );
}

#[test]
fn double_purchase_rejected() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE)
        .unwrap();
    // Second purchase attempts to buy an already-removed listing.
    assert_eq!(
        ctx.nft.buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE),
        Err(crate::types::Error::ListingNotFound)
    );
}

#[test]
#[should_panic]
fn buy_with_insufficient_balance_rejected() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    // A buyer with no funds attempts to purchase.
    let broke = Address::generate(&ctx.env);
    ctx.nft
        .buy_listing(&broke, &ctx.token_id, &ctx.token, &LISTING_PRICE);
}

#[test]
fn royalty_enforced_on_purchase() {
    let ctx = setup(0);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE)
        .unwrap();
    // 5% royalty should reach the recipient.
    assert_eq!(
        ctx.token_client.balance(&ctx.royalty_recipient),
        LISTING_PRICE * 500 / 10_000
    );
}

#[test]
fn platform_fee_enforced_on_purchase() {
    let ctx = setup(250);
    ctx.nft.create_listing(&listing(&ctx)).unwrap();
    ctx.nft
        .buy_listing(&ctx.buyer, &ctx.token_id, &ctx.token, &LISTING_PRICE)
        .unwrap();
    // 2.5% platform fee collected by the contract.
    assert_eq!(
        ctx.token_client.balance(&ctx.nft.address),
        LISTING_PRICE * 250 / 10_000
    );
}

// ─── Offer flow ──────────────────────────────────────────────────────────────

#[test]
fn make_offer_success() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    assert!(emitted(&ctx.env, "ofr_made"));
}

#[test]
fn duplicate_offer_rejected() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    assert_eq!(
        ctx.nft.make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0),
        Err(crate::types::Error::OfferAlreadyExists)
    );
}

#[test]
fn expired_offer_rejected() {
    let ctx = setup(0);
    assert_eq!(
        ctx.nft.make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &100),
        Err(crate::types::Error::OfferExpired)
    );
}

#[test]
fn accept_offer_transfers_nft_and_settles() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    ctx.nft.accept_offer(&ctx.seller, &ctx.token_id).unwrap();
    assert!(emitted(&ctx.env, "ofr_acc"));

    assert_eq!(ctx.nft.owner_of(&ctx.token_id).unwrap(), ctx.buyer);
    let expected_royalty = LISTING_PRICE * 500 / 10_000;
    assert_eq!(ctx.token_client.balance(&ctx.royalty_recipient), expected_royalty);
    assert_eq!(ctx.token_client.balance(&ctx.seller), LISTING_PRICE - expected_royalty);
}

#[test]
fn accept_offer_requires_owner() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    // Attacker (not the owner) attempts to accept.
    assert_eq!(
        ctx.nft.accept_offer(&ctx.attacker, &ctx.token_id),
        Err(crate::types::Error::Unauthorized)
    );
}

#[test]
fn cancel_offer_requires_buyer() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    assert_eq!(
        ctx.nft.cancel_offer(&ctx.attacker, &ctx.token_id),
        Err(crate::types::Error::Unauthorized)
    );
}

#[test]
fn cancel_offer_emits_event() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    ctx.nft.cancel_offer(&ctx.buyer, &ctx.token_id).unwrap();
    assert!(emitted(&ctx.env, "ofr_can"));
    // Offer is gone, so a second cancel fails.
    assert_eq!(
        ctx.nft.cancel_offer(&ctx.buyer, &ctx.token_id),
        Err(crate::types::Error::TokenNotFound)
    );
}

#[test]
fn double_offer_accept_rejected() {
    let ctx = setup(0);
    ctx.nft
        .make_offer(&ctx.buyer, &ctx.token_id, &LISTING_PRICE, &ctx.token, &0)
        .unwrap();
    ctx.nft.accept_offer(&ctx.seller, &ctx.token_id).unwrap();
    // After acceptance the offer is removed; re-accepting fails.
    assert_eq!(
        ctx.nft.accept_offer(&ctx.seller, &ctx.token_id),
        Err(crate::types::Error::TokenNotFound)
    );
}
