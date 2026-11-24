#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Vec,
};

use clips_nft::{
    errors::Error,
    ClipsNftClient,
};

fn setup_env() -> (Env, ClipsNftClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, clips_nft::ClipsNft);
    let client = ClipsNftClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin, owner)
}

fn mint_token(
    env: &Env,
    client: &ClipsNftClient,
    owner: &Address,
) -> u32 {
    let token_id = client.mint(owner, &soroban_sdk::String::from_str(env, "ipfs://token"));
    token_id
}

#[test]
fn test_unauthorized_transfer() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let unauthorized = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Attempt transfer from an address that does not own the token.
    let result = client.try_transfer(&unauthorized, &recipient, &token_id);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // Owner should still hold the token.
    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_invalid_operator() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let invalid_operator = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Operator was never approved; approve_for semantics should reject.
    let result = client.try_transfer_from(&invalid_operator, &owner, &recipient, &token_id);
    assert_eq!(result, Err(Ok(Error::InvalidOperator)));

    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_missing_approval() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let operator = Address::generate(&env);
    let recipient = Address::generate(&env);

    // No approval was granted, so transfer_from must fail with MissingApproval.
    let result = client.try_transfer_from(&operator, &owner, &recipient, &token_id);
    assert_eq!(result, Err(Ok(Error::MissingApproval)));

    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_duplicate_approval() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let operator = Address::generate(&env);

    // First approval succeeds.
    client.approve(&owner, &operator, &token_id);

    // Second approval for the same operator should be rejected as duplicate.
    let result = client.try_approve(&owner, &operator, &token_id);
    assert_eq!(result, Err(Ok(Error::DuplicateApproval)));
}

#[test]
fn test_frozen_nft_transfer() {
    let (env, client, admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let recipient = Address::generate(&env);

    // Freeze the token (admin action).
    client.freeze(&admin, &token_id);

    let result = client.try_transfer(&owner, &recipient, &token_id);
    assert_eq!(result, Err(Ok(Error::Frozen)));

    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_burned_nft_transfer() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let recipient = Address::generate(&env);

    // Burn the token.
    client.burn(&owner, &token_id);

    // Transferring a burned token must fail with TokenNotFound / Burned.
    let result = client.try_transfer(&owner, &recipient, &token_id);
    assert_eq!(result, Err(Ok(Error::Burned)));
}

#[test]
fn test_self_transfer() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);

    // Transferring to oneself should be rejected.
    let result = client.try_transfer(&owner, &owner, &token_id);
    assert_eq!(result, Err(Ok(Error::SelfTransfer)));

    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_invalid_batch_transfer() {
    let (env, client, _admin, owner) = setup_env();
    let token_a = mint_token(&env, &client, &owner);
    let token_b = mint_token(&env, &client, &owner);
    let recipient = Address::generate(&env);

    // Build a batch containing a token the sender does not own.
    let stranger = Address::generate(&env);
    let foreign_token = mint_token(&env, &client, &stranger);

    let mut tokens: Vec<u32> = Vec::new(&env);
    tokens.push_back(token_a);
    tokens.push_back(foreign_token);

    let result = client.try_batch_transfer(&owner, &recipient, &tokens);
    assert_eq!(result, Err(Ok(Error::InvalidBatch)));

    // Ensure no partial state change occurred.
    assert_eq!(client.owner_of(&token_a), owner);
    assert_eq!(client.owner_of(&token_b), owner);
    assert_eq!(client.owner_of(&foreign_token), stranger);
}

#[test]
fn test_invalid_batch_empty() {
    let (env, client, _admin, owner) = setup_env();
    let recipient = Address::generate(&env);

    // Empty batch should be rejected as invalid.
    let tokens: Vec<u32> = Vec::new(&env);
    let result = client.try_batch_transfer(&owner, &recipient, &tokens);
    assert_eq!(result, Err(Ok(Error::InvalidBatch)));
}

#[test]
fn test_invalid_batch_duplicate_tokens() {
    let (env, client, _admin, owner) = setup_env();
    let token_id = mint_token(&env, &client, &owner);
    let recipient = Address::generate(&env);

    // Duplicate token ids in a batch should be rejected.
    let mut tokens: Vec<u32> = Vec::new(&env);
    tokens.push_back(token_id);
    tokens.push_back(token_id);

    let result = client.try_batch_transfer(&owner, &recipient, &tokens);
    assert_eq!(result, Err(Ok(Error::InvalidBatch)));

    assert_eq!(client.owner_of(&token_id), owner);
}
use clips_nft::{BatchTransferError, ClipsNftContract, ClipsNftContractClient};
use soroban_sdk::{testutils::Address as _, token, Address, Env, Vec};

fn setup_env() -> (Env, ClipsNftContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, ClipsNftContract);
    let client = ClipsNftContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let recipient = Address::generate(&env);

    client.initialize(&admin);

    (env, client, admin, recipient)
}

#[test]
fn test_batch_transfer_empty_batch() {
    let (env, client, _admin, _recipient) = setup_env();

    let empty: Vec<u32> = Vec::new(&env);
    let recipient = Address::generate(&env);

    let result = client.try_batch_transfer(&empty, &recipient);

    assert_eq!(result, Err(Ok(BatchTransferError::EmptyBatch)));
}

#[test]
fn test_batch_transfer_oversized_batch() {
    let (env, client, _admin, _recipient) = setup_env();

    let max_batch_size = client.max_batch_size();
    let mut token_ids: Vec<u32> = Vec::new(&env);
    for i in 0..(max_batch_size + 1) {
        token_ids.push_back(i);
    }

    let recipient = Address::generate(&env);

    let result = client.try_batch_transfer(&token_ids, &recipient);

    assert_eq!(result, Err(Ok(BatchTransferError::OversizedBatch)));
}

#[test]
fn test_batch_transfer_duplicate_token() {
    let (env, client, admin, _recipient) = setup_env();

    let token_id = client.mint(&admin);
    let recipient = Address::generate(&env);

    let mut token_ids: Vec<u32> = Vec::new(&env);
    token_ids.push_back(token_id);
    token_ids.push_back(token_id);

    let result = client.try_batch_transfer(&token_ids, &recipient);

    assert_eq!(result, Err(Ok(BatchTransferError::DuplicateToken)));
}

#[test]
fn test_batch_transfer_invalid_recipient() {
    let (env, client, admin, _recipient) = setup_env();

    let token_id = client.mint(&admin);

    let mut token_ids: Vec<u32> = Vec::new(&env);
    token_ids.push_back(token_id);

    // Use the contract address itself as an invalid recipient.
    let invalid_recipient = client.address.clone();

    let result = client.try_batch_transfer(&token_ids, &invalid_recipient);

    assert_eq!(result, Err(Ok(BatchTransferError::InvalidRecipient)));
}

#[test]
fn test_batch_transfer_failed_transfer() {
    let (env, client, admin, _recipient) = setup_env();

    // Token that does not exist / is not owned by the caller.
    let mut token_ids: Vec<u32> = Vec::new(&env);
    token_ids.push_back(9999);

    let recipient = Address::generate(&env);

    let result = client.try_batch_transfer(&token_ids, &recipient);

    assert_eq!(result, Err(Ok(BatchTransferError::FailedTransfer)));
}

#[test]
fn test_batch_transfer_success() {
    let (env, client, admin, _recipient) = setup_env();

    let token_a = client.mint(&admin);
    let token_b = client.mint(&admin);

    let mut token_ids: Vec<u32> = Vec::new(&env);
    token_ids.push_back(token_a);
    token_ids.push_back(token_b);

    let recipient = Address::generate(&env);

    let result = client.try_batch_transfer(&token_ids, &recipient);
    assert!(result.is_ok());

    assert_eq!(client.owner_of(&token_a), recipient);
    assert_eq!(client.owner_of(&token_b), recipient);
}

// Silence unused import warning for token in environments where it is not used.
#[allow(dead_code)]
fn _unused(_: token::Client) {}
