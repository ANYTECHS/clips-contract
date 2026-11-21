#![cfg(test)]

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