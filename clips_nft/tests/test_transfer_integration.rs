#![cfg(test)]

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, NFTFrozenEvent};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, LedgerInfo},
    Address, Env, String, Vec,
};

use test_helpers::{mint_clip, setup, TestContext};

#[test]
fn test_successful_single_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 1, false);

    assert_eq!(ctx.client.owner_of(&token_id), owner);

    ctx.client
        .transfer(&owner, &recipient, &token_id, &0i128, &None);

    assert_eq!(ctx.client.owner_of(&token_id), recipient);
}

#[test]
fn test_unauthorized_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let malicious = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 2, false);

    let res = ctx
        .client
        .try_transfer(&malicious, &owner, &token_id, &0i128, &None);
    assert!(res.is_err());
}

#[test]
fn test_frozen_nft() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    // is_soulbound = true -> frozen
    let token_id = mint_clip(&ctx, &owner, 3, true);

    let res = ctx
        .client
        .try_transfer(&owner, &recipient, &token_id, &0i128, &None);
    assert!(res.is_err());
}

#[test]
fn test_freeze_emits_event_with_token_caller_reason_and_timestamp() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 30, false);
    let reason = String::from_str(ctx.env, "investigating wallet compromise");

    ctx.env.ledger().set(LedgerInfo {
        timestamp: 1_720_000_000,
        protocol_version: 21,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 3_110_400,
    });

    ctx.client.freeze_token(&ctx.admin, &token_id, &Some(reason.clone()));

    let event = ctx
        .env
        .events()
        .all()
        .events()
        .iter()
        .filter_map(|(_, data): (Vec<soroban_sdk::Val>, NFTFrozenEvent)| Some(data))
        .find(|data| data.token_id == token_id)
        .expect("NFTFrozenEvent not found");

    assert_eq!(event.token_id, token_id);
    assert_eq!(event.caller, ctx.admin);
    assert_eq!(event.reason, Some(reason));
    assert_eq!(event.timestamp, 1_720_000_000);
}

#[test]
fn test_blacklisted_wallet() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let bad_actor = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 4, false);

    ctx.client.blacklist_wallet(&ctx.admin, &bad_actor);

    let res = ctx
        .client
        .try_transfer(&owner, &bad_actor, &token_id, &0i128, &None);
    assert!(res.is_err());
}

#[test]
fn test_approved_operator_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let operator = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 5, false);

    ctx.client
        .approve(&owner, &Some(operator.clone()), &token_id);

    ctx.client
        .transfer_from(&operator, &owner, &recipient, &token_id);

    assert_eq!(ctx.client.owner_of(&token_id), recipient);
    assert_eq!(ctx.client.get_approved(&token_id), None); // Approval cleanup
}

#[test]
fn test_event_emission_on_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 6, false);

    ctx.client
        .transfer(&owner, &recipient, &token_id, &0i128, &None);

    let events = ctx.env.events().all();
    assert!(events.len() > 0);
}

#[test]
fn test_invalid_recipient() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 7, false);

    // Transfer to self should fail or be a no-op depending on impl. Assuming it fails as invalid recipient.
    let res = ctx
        .client
        .try_transfer(&owner, &owner, &token_id, &0i128, &None);
    assert!(res.is_err());
}
