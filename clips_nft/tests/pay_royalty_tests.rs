//! Comprehensive tests for royalty payment distribution.

mod test_helpers;

use clips_nft::{Error, Royalty, RoyaltyPaidEvent, RoyaltyRecipient};
use soroban_sdk::{testutils::{Address as _, Events}, token, Address, Env, Vec, IntoVal};
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
fn test_successful_payment_and_event_emission() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 10_000_000);

    let token_id = mint_clip(&ctx, &creator, 901, false);
    let royalty = royalty_with_asset(ctx.env, creator.clone(), 500, asset.clone());
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    let sale_price = 2_000_000i128; // 5% = 100,000
    
    // Clear events
    ctx.env.events().all(); 

    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);

    let balance = token::TokenClient::new(ctx.env, &asset).balance(&creator);
    assert_eq!(balance, 100_000);
    
    // Earnings counter
    assert_eq!(ctx.client.get_cumulative_earnings(&token_id), 100_000);
    
    // Event emission check
    let events = ctx.env.events().all();
    assert!(!events.is_empty());
    
    let (_, topics, data) = events.last().unwrap();
    let expected_topic0: soroban_sdk::Symbol = soroban_sdk::Symbol::new(ctx.env, "royalty");
    assert_eq!(topics.get(0).unwrap(), expected_topic0.into_val(ctx.env));
    assert_eq!(topics.get(1).unwrap(), token_id.into_val(ctx.env));
    
    let event: RoyaltyPaidEvent = data.try_into_val(ctx.env).unwrap();
    assert_eq!(event.token_id, token_id);
    assert_eq!(event.payer, buyer);
    assert_eq!(event.receiver, creator);
    assert_eq!(event.amount, 100_000);
    assert_eq!(event.asset_address, Some(asset));
    assert_eq!(event.timestamp, ctx.env.ledger().timestamp());
}

#[test]
fn test_zero_royalty() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 10_000_000);

    let token_id = mint_clip(&ctx, &creator, 902, false);
    // 0 basis points
    let royalty = royalty_with_asset(ctx.env, creator.clone(), 0, asset.clone());
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    let sale_price = 1_000_000i128;
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);

    // No funds transferred
    let balance = token::TokenClient::new(ctx.env, &asset).balance(&creator);
    assert_eq!(balance, 0);
    
    // Earnings counter should remain 0
    assert_eq!(ctx.client.get_cumulative_earnings(&token_id), 0);
}

#[test]
fn test_invalid_asset() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    
    let token_id = mint_clip(&ctx, &creator, 903, false);
    let mut recipients = Vec::new(ctx.env);
    recipients.push_back(RoyaltyRecipient {
        recipient: creator.clone(),
        basis_points: 500,
    });
    
    // Asset address is None for native XLM but pay_royalty rejects it if amount > 0 and no asset
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    let sale_price = 1_000_000i128;
    let result = ctx.client.try_pay_royalty(&buyer, &token_id, &sale_price);
    assert_eq!(result, Err(Ok(Error::InvalidConfig)));
}

#[test]
fn test_duplicate_payment() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, 10_000_000);

    let token_id = mint_clip(&ctx, &creator, 905, false);
    let royalty = royalty_with_asset(ctx.env, creator.clone(), 500, asset.clone());
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    let sale_price = 2_000_000i128;
    
    // First payment succeeds
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);
    assert_eq!(ctx.client.get_cumulative_earnings(&token_id), 100_000);

    // Second payment fails
    let result = ctx.client.try_pay_royalty(&buyer, &token_id, &sale_price);
    assert_eq!(result, Err(Ok(Error::PaymentAlreadyProcessed)));
    
    // Earnings should not increment on failure
    assert_eq!(ctx.client.get_cumulative_earnings(&token_id), 100_000);
}

#[test]
fn test_large_amount() {
    let ctx = setup();
    let creator = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);
    let asset = deploy_token(ctx.env, &buyer, i128::MAX);

    let token_id = mint_clip(&ctx, &creator, 906, false);
    let royalty = royalty_with_asset(ctx.env, creator.clone(), 500, asset.clone());
    ctx.client.set_royalty(&ctx.admin, &token_id, &royalty);

    let sale_price = 1_000_000_000_000i128;
    
    ctx.client.pay_royalty(&buyer, &token_id, &sale_price);
    
    let expected_amount = 50_000_000_000i128;
    let balance = token::TokenClient::new(ctx.env, &asset).balance(&creator);
    assert_eq!(balance, expected_amount);
    
    assert_eq!(ctx.client.get_cumulative_earnings(&token_id), expected_amount);
}
