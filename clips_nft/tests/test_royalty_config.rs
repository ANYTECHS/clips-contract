#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

use clips_nft::{ClipsNftContract, ClipsNftContractClient, RoyaltyError};

fn setup() -> (Env, ClipsNftContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, ClipsNftContract);
    let client = ClipsNftContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin, owner)
}

#[test]
fn test_royalty_above_maximum() {
    let (env, client, _admin, owner) = setup();
    let recipient = Address::generate(&env);
    let token_id = client.mint(&owner);

    // BPS above the maximum allowed (e.g. 10000 = 100%) must be rejected.
    let result = client.try_set_royalty(&owner, &token_id, &recipient, &10_001u32);
    assert_eq!(result, Err(Ok(RoyaltyError::RoyaltyAboveMaximum)));
}

#[test]
fn test_invalid_recipient() {
    let (env, client, _admin, owner) = setup();
    let token_id = client.mint(&owner);

    // The contract address itself is not a valid royalty recipient.
    let invalid_recipient = client.address.clone();
    let result = client.try_set_royalty(&owner, &token_id, &invalid_recipient, &500u32);
    assert_eq!(result, Err(Ok(RoyaltyError::InvalidRecipient)));
}

#[test]
fn test_invalid_bps() {
    let (env, client, _admin, owner) = setup();
    let recipient = Address::generate(&env);
    let token_id = client.mint(&owner);

    // Zero BPS is not a valid royalty configuration.
    let result = client.try_set_royalty(&owner, &token_id, &recipient, &0u32);
    assert_eq!(result, Err(Ok(RoyaltyError::InvalidBps)));
}

#[test]
fn test_invalid_royalty_state() {
    let (env, client, _admin, owner) = setup();
    let recipient = Address::generate(&env);

    // Setting royalty on a non-existent token is an invalid state.
    let missing_token_id = 999u32;
    let result = client.try_set_royalty(&owner, &missing_token_id, &recipient, &500u32);
    assert_eq!(result, Err(Ok(RoyaltyError::InvalidRoyaltyState)));
}

#[test]
fn test_unauthorized_update() {
    let (env, client, _admin, owner) = setup();
    let recipient = Address::generate(&env);
    let attacker = Address::generate(&env);
    let token_id = client.mint(&owner);

    // A non-owner must not be able to update the royalty configuration.
    let result = client.try_set_royalty(&attacker, &token_id, &recipient, &500u32);
    assert_eq!(result, Err(Ok(RoyaltyError::UnauthorizedUpdate)));
}