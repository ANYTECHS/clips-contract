//! Integration tests for SEP-0041 mock token royalty flows.

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Error, Royalty, RoyaltyRecipient};
use soroban_sdk::{testutils::Address as _, token, Address, Env, String, Vec};
use test_helpers::*;

fn royalty_with_asset(env: &Env, recipient: Address, bps: u32, asset: Address) -> Royalty {
    let mut recipients = Vec::new(env);
    recipients.push_back(RoyaltyRecipient {
        recipient,
        basis_points: bps,
    });
    Royalty {
        recipients,
        asset_address: Some(asset),
    }
}

#[test]
fn test_sep41_pay_royalty_single_recipient() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 10_000_000);

    let token_id = mint_clip(&ctx, &creator, 501, false);
    ctx.client.set_royalty(
        &ctx.admin,
        &token_id,
        &royalty_with_asset(ctx.env, creator.clone(), 500, asset.clone()),
    );

    let sale_price = 2_000_000i128;
    let info = ctx.client.royalty_info(&token_id, &sale_price);
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);

    let balance = token::TokenClient::new(ctx.env, &asset).balance(&creator);
    assert!(balance > 0);
    assert!(balance <= info.royalty_amount);
}

#[test]
fn test_sep41_pay_royalty_multi_recipient_split() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let platform = ctx.admin.clone();
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 10_000_000);

    let uri = String::from_str(ctx.env, "ipfs://QmMultiRoyalty");
    let sig = sign_mint(ctx.env, &ctx.keypair, &creator, 502, &uri);
    let mut recipients = Vec::new(ctx.env);
    recipients.push_back(RoyaltyRecipient {
        recipient: creator.clone(),
        basis_points: 500,
    });
    recipients.push_back(RoyaltyRecipient {
        recipient: platform.clone(),
        basis_points: 300,
    });
    let royalty = Royalty {
        recipients,
        asset_address: Some(asset.clone()),
    };
    let token_id = ctx
        .client
        .mint(&creator, &502, &uri, &None, &None, &royalty, &false, &None, &sig);

    let sale_price = 1_000_000i128;
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);

    let token = token::TokenClient::new(ctx.env, &asset);
    assert!(token.balance(&creator) > 0);
    assert!(token.balance(&platform) > 0);
}

#[test]
fn test_sep41_secondary_sale_buyer_pays_royalty_then_transfer() {
    let ctx = setup();
    let seller = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 5_000_000);

    let token_id = mint_clip(&ctx, &seller, 503, false);
    let mut royalty = ctx.client.get_royalty(&token_id);
    royalty.asset_address = Some(asset.clone());
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    let sale_price = 1_000_000i128;
    let info = ctx.client.royalty_info(&token_id, &sale_price);
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);
    ctx.client.transfer(&seller, &buyer, &token_id, &0, &None);

    assert_eq!(ctx.client.owner_of(&token_id), buyer);
    assert!(info.royalty_amount > 0);
    assert!(token::TokenClient::new(ctx.env, &asset).balance(&seller) > 0);
}

#[test]
fn test_sep41_pay_royalty_zero_sale_price_fails() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 1_000);

    let token_id = mint_clip(&ctx, &creator, 504, false);
    ctx.client.set_royalty(
        &ctx.admin,
        &token_id,
        &royalty_with_asset(ctx.env, creator, 500, asset),
    );

    let result = ctx.client.try_pay_royalty(&buyer, &token_id, &0i128);
    assert_eq!(result, Err(Ok(Error::InvalidSalePrice)));
}

#[test]
fn test_sep41_pay_royalty_xlm_config_fails() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &creator, 505, false);

    let result = ctx
        .client
        .try_pay_royalty(&buyer, &token_id, &1_000_000i128);
    assert_eq!(result, Err(Ok(Error::InvalidRecipient)));
}

#[test]
fn test_sep41_royalty_info_matches_paid_total() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 20_000_000);

    let token_id = mint_clip(&ctx, &creator, 506, false);
    ctx.client.set_royalty(
        &ctx.admin,
        &token_id,
        &royalty_with_asset(ctx.env, creator.clone(), 1000, asset.clone()),
    );

    let sale_price = 500_000i128;
    let info = ctx.client.royalty_info(&token_id, &sale_price);
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);

    let creator_balance = token::TokenClient::new(ctx.env, &asset).balance(&creator);
    assert!(creator_balance > 0);
    assert!(creator_balance <= info.royalty_amount);
}

#[test]
fn test_transfer_with_royalty_enforcement_custom_asset() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let seller = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 10_000_000);

    // Mint token for seller
    let token_id = mint_clip(&ctx, &seller, 601, false);

    // Set royalty with custom asset
    let mut recipients = Vec::new(ctx.env);
    recipients.push_back(RoyaltyRecipient {
        recipient: creator.clone(),
        basis_points: 500, // 5%
    });
    let royalty = Royalty {
        recipients,
        asset_address: Some(asset.clone()),
    };
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    // Execute transfer with royalty enforcement
    let sale_price = 1_000_000i128;
    ctx.client.transfer(
        &seller,
        &buyer,
        &token_id,
        &sale_price,
        &Some(asset.clone()),
    );

    // Verify ownership
    assert_eq!(ctx.client.owner_of(&token_id), buyer);

    // Verify royalty payment
    // Creator should get 5% of 1_000_000 = 50_000
    let token_client = token::TokenClient::new(ctx.env, &asset);
    assert_eq!(token_client.balance(&creator), 50_000);

    // Platform (admin) should get 1% of 1_000_000 = 10_000
    assert_eq!(token_client.balance(&ctx.admin), 10_000);
}

#[test]
fn test_transfer_with_royalty_enforcement_xlm() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let seller = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let mock_xlm = deploy_token(ctx.env, &buyer, 10_000_000);

    // Mint token for seller
    let token_id = mint_clip(&ctx, &seller, 602, false);

    // Set royalty with None asset (XLM)
    let mut recipients = Vec::new(ctx.env);
    recipients.push_back(RoyaltyRecipient {
        recipient: creator.clone(),
        basis_points: 500, // 5%
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    // Execute transfer with royalty enforcement
    let sale_price = 1_000_000i128;
    ctx.client.transfer(
        &seller,
        &buyer,
        &token_id,
        &sale_price,
        &Some(mock_xlm.clone()),
    );

    // Verify ownership
    assert_eq!(ctx.client.owner_of(&token_id), buyer);

    // Verify royalty payment
    let token_client = token::TokenClient::new(ctx.env, &mock_xlm);
    assert_eq!(token_client.balance(&creator), 50_000);
    assert_eq!(token_client.balance(&ctx.admin), 10_000);
}
