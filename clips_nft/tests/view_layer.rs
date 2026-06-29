#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, Env, String, Vec,
};

mod test_helpers;
use test_helpers::*;

use clips_nft::{Error, Royalty};

#[test]
fn test_view_layer_read_only_functions_report_expected_state() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 1000, false);

    assert_eq!(ctx.client.owner_of(&token_id), owner);
    assert_eq!(ctx.client.balance_of(&owner), 1);
    assert_eq!(ctx.client.token_uri(&token_id), String::from_str(ctx.env, "ipfs://QmClip1000"));
    assert_eq!(ctx.client.get_metadata(&token_id), ctx.client.token_uri(&token_id));

    let json = ctx.client.get_metadata_json(&token_id);
    assert!(format!("{json}").contains("ipfs://QmClip1000"));

    assert_eq!(ctx.client.clip_token_id(&1000u32), token_id);

    let mut clip_ids = Vec::new(ctx.env);
    clip_ids.push_back(1000u32);
    clip_ids.push_back(2_000u32);
    let tokens = ctx.client.get_tokens_by_clip_ids(&clip_ids);
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens.get(0).unwrap().unwrap(), token_id);
    assert!(tokens.get(1).unwrap().is_none());

    assert_eq!(ctx.client.total_supply(), 1);
    assert_eq!(ctx.client.minted_count(), 1);
    assert!(!ctx.client.is_soulbound(&token_id));
    assert_eq!(ctx.client.royalty_balance_of(&token_id), 0);

    assert!(!ctx.client.circuit_breaker_enabled());
    assert_eq!(ctx.client.circuit_breaker_threshold(), 100);
    assert_eq!(ctx.client.circuit_breaker_window_seconds(), 60);
    assert_eq!(ctx.client.circuit_breaker_window_start(), 0);
    assert_eq!(ctx.client.circuit_breaker_window_count(), 0);

    assert_eq!(ctx.client.get_next_metadata_refresh_time(&token_id), 0);
}

#[test]
fn test_view_layer_enumeration_metrics_and_royalty_balance() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let buyer = Address::generate(ctx.env);

    let first = mint_clip(&ctx, &owner, 1010, false);
    let second = mint_clip(&ctx, &owner, 1011, false);
    let third = mint_clip(&ctx, &buyer, 1012, false);

    assert_eq!(ctx.client.balance_of(&owner), 2);
    assert_eq!(ctx.client.balance_of(&buyer), 1);
    assert_eq!(ctx.client.total_supply(), 3);
    assert_eq!(ctx.client.minted_count(), 3);

    assert_eq!(ctx.client.token_by_index(&0u32), first);
    assert_eq!(ctx.client.token_by_index(&1u32), second);
    assert_eq!(ctx.client.token_by_index(&2u32), third);

    assert_eq!(ctx.client.token_of_owner_by_index(&owner, &0u32), first);
    assert_eq!(ctx.client.token_of_owner_by_index(&owner, &1u32), second);

    assert!(ctx.client.average_gas_mint() > 0);
    assert_eq!(ctx.client.total_mints(), 3);

    let asset = deploy_token(ctx.env, &buyer, 10_000);
    let mut royalty = ctx.client.get_royalty(&third);
    royalty.asset_address = Some(asset.clone());
    ctx.client.set_royalty(&ctx.admin, &third, &royalty);

    let sale_price = 1_000i128;
    let info = ctx.client.royalty_info(&third, &sale_price);
    ctx.client.pay_royalty(&buyer, &third, &sale_price);

    assert_eq!(ctx.client.royalty_balance_of(&third), info.royalty_amount);

    ctx.client.transfer(&buyer, &owner, &third, &0i128, &None);
    assert_eq!(ctx.client.total_transfers(), 1);
    assert!(ctx.client.average_gas_transfer() > 0);

    assert_eq!(ctx.client.balance_of(&owner), 3);
    assert_eq!(ctx.client.token_of_owner_by_index(&owner, &2u32), third);
}

#[test]
fn test_view_layer_invalid_queries_return_errors() {
    let ctx = setup_test();
    let owner = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 1020, false);

    assert_eq!(ctx.client.try_get_next_metadata_refresh_time(&9999u32), Err(Ok(Error::InvalidTokenId)));
    assert_eq!(ctx.client.try_token_of_owner_by_index(&owner, &5u32), Err(Ok(Error::InvalidTokenId)));
    assert_eq!(ctx.client.try_royalty_info(&token_id, &0i128), Err(Ok(Error::InvalidSalePrice)));
}
