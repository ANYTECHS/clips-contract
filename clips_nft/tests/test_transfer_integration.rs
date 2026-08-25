#![cfg(test)]

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient};
use soroban_sdk::{testutils::{Address as _, Events}, Address, Env, String};

use test_helpers::{setup, mint_clip, TestContext};

#[test]
fn test_successful_single_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 1, false);

    assert_eq!(ctx.client.owner_of(&token_id), owner);

    ctx.client.transfer(&owner, &recipient, &token_id, &0i128, &None);

    assert_eq!(ctx.client.owner_of(&token_id), recipient);
}

#[test]
fn test_unauthorized_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let malicious = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 2, false);

    let res = ctx.client.try_transfer(&malicious, &owner, &token_id, &0i128, &None);
    assert!(res.is_err());
}

#[test]
fn test_frozen_nft() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    // is_soulbound = true -> frozen
    let token_id = mint_clip(&ctx, &owner, 3, true);

    let res = ctx.client.try_transfer(&owner, &recipient, &token_id, &0i128, &None);
    assert!(res.is_err());
}

#[test]
fn test_blacklisted_wallet() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let bad_actor = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 4, false);

    ctx.client.blacklist_wallet(&ctx.admin, &bad_actor);
    
    let res = ctx.client.try_transfer(&owner, &bad_actor, &token_id, &0i128, &None);
    assert!(res.is_err());
}

#[test]
fn test_approved_operator_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let operator = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 5, false);

    ctx.client.approve(&owner, &Some(operator.clone()), &token_id);
    
    ctx.client.transfer_from(&operator, &owner, &recipient, &token_id);
    
    assert_eq!(ctx.client.owner_of(&token_id), recipient);
    assert_eq!(ctx.client.get_approved(&token_id), None); // Approval cleanup
}

#[test]
fn test_event_emission_on_transfer() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let recipient = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 6, false);

    ctx.client.transfer(&owner, &recipient, &token_id, &0i128, &None);

    let events = ctx.env.events().all();
    assert!(events.len() > 0);
}

#[test]
fn test_invalid_recipient() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 7, false);

    // Transfer to self should fail or be a no-op depending on impl. Assuming it fails as invalid recipient.
    let res = ctx.client.try_transfer(&owner, &owner, &token_id, &0i128, &None);
    assert!(res.is_err());
}
