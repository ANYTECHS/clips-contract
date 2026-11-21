#![cfg(test)]

use crate::{ClipsNftContract, ClipsNftContractClient};
use crate::transfer_request::TransferRequest;
use crate::types::TokenId;
use crate::errors::ContractError;
use soroban_sdk::{testutils::{Address as _, BytesN as _}, Address, Env, Vec};

// We will use the AtomicMintContract to mint, but we must use ClipsNftContract to transfer.
use crate::AtomicMintContract;
use crate::atomic_mint::AtomicMintContractClient;
use crate::atomic_mint::MintParams;
use crate::signature_replay_storage::hash_signature;
use soroban_sdk::{BytesN, String};
use crate::types::{Royalty, RoyaltyRecipient};

fn setup() -> (Env, ClipsNftContractClient<'static>, AtomicMintContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    
    // Register the main contract (for transfers)
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    client.init(&admin);

    // Register atomic mint contract
    let mint_contract_id = env.register(AtomicMintContract, ());
    let mint_client = AtomicMintContractClient::new(&env, &mint_contract_id);
    mint_client.init(&admin);
    
    (env, client, mint_client)
}

fn mint_token(env: &Env, client: &AtomicMintContractClient, owner: &Address, clip_id: u32) -> TokenId {
    let sig = BytesN::<64>::generate(env);
    let mut recipients = Vec::new(env);
    recipients.push_back(RoyaltyRecipient {
        recipient: owner.clone(),
        basis_points: 0,
    });
    let params = MintParams {
        owner: owner.clone(),
        clip_id,
        metadata_uri: String::from_str(env, "ipfs://test"),
        royalty: Royalty {
            recipients,
            asset_address: None,
        },
        signature_hash: hash_signature(env, &sig),
        creator_address: None,
        creator_display_name: None,
    };
    client.mint(&params)
}

#[test]
fn integration_test_transfer() {
    let (env, client, mint_client) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    // Mint token 0 to 'from'
    let token_id = mint_token(&env, &mint_client, &from, 100);
    
    // Check initial owner
    assert_eq!(mint_client.owner_of(&token_id), from);
    assert_eq!(mint_client.tokens_of_owner(&from).len(), 1);
    
    // Execute transfer using the current 4-argument signature
    client.transfer(&from, &from, &to, &token_id);

    // Verify owner changed
    assert_eq!(mint_client.owner_of(&token_id), to);
    assert_eq!(mint_client.tokens_of_owner(&from).len(), 0);
    assert_eq!(mint_client.tokens_of_owner(&to).len(), 1);
}

#[test]
fn integration_test_batch_transfer() {
    let (env, client, mint_client) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    let token_1 = mint_token(&env, &mint_client, &from, 201);
    let token_2 = mint_token(&env, &mint_client, &from, 202);
    
    let mut requests = Vec::new(&env);
    requests.push_back(TransferRequest {
        from: from.clone(),
        to: to.clone(),
        token_id: token_1,
        timestamp: 0,
        memo: None,
    });
    requests.push_back(TransferRequest {
        from: from.clone(),
        to: to.clone(),
        token_id: token_2,
        timestamp: 0,
        memo: None,
    });

    client.batch_transfer(&from, &requests);

    assert_eq!(mint_client.owner_of(&token_1), to);
    assert_eq!(mint_client.owner_of(&token_2), to);
    assert_eq!(mint_client.tokens_of_owner(&from).len(), 0);
    assert_eq!(mint_client.tokens_of_owner(&to).len(), 2);
}

// ---------------------------------------------------------------------------
// Error scenario tests
// ---------------------------------------------------------------------------

#[test]
fn test_transfer_unauthorized() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    let to = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 300);

    // Attacker is not the owner and has no approval.
    let result = client.try_transfer(&attacker, &attacker, &to, &token_id);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));

    // Ownership must remain unchanged.
    assert_eq!(mint_client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_invalid_operator() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);
    let to = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 301);

    // Operator was never approved for this token.
    let result = client.try_transfer(&operator, &owner, &to, &token_id);
    assert_eq!(result, Err(Ok(ContractError::InvalidOperator)));

    assert_eq!(mint_client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_missing_approval() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);
    let to = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 302);

    // No approval exists for the operator.
    assert_eq!(client.get_approved(&token_id), None);

    let result = client.try_transfer(&operator, &owner, &to, &token_id);
    assert_eq!(result, Err(Ok(ContractError::MissingApproval)));

    assert_eq!(mint_client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_duplicate_approval() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 303);

    // First approval succeeds.
    client.approve(&owner, &operator, &token_id);
    assert_eq!(client.get_approved(&token_id), Some(operator.clone()));

    // Approving the same operator again is a duplicate approval.
    let result = client.try_approve(&owner, &operator, &token_id);
    assert_eq!(result, Err(Ok(ContractError::DuplicateApproval)));
}

#[test]
fn test_transfer_frozen_nft() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 304);

    // Freeze the token.
    client.freeze(&owner, &token_id);

    let result = client.try_transfer(&owner, &owner, &to, &token_id);
    assert_eq!(result, Err(Ok(ContractError::TokenFrozen)));

    assert_eq!(mint_client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_burned_nft() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 305);

    // Burn the token.
    client.burn(&owner, &token_id);

    let result = client.try_transfer(&owner, &owner, &to, &token_id);
    assert_eq!(result, Err(Ok(ContractError::TokenBurned)));
}

#[test]
fn test_transfer_self_transfer() {
    let (env, client, mint_client) = setup();
    let owner = Address::generate(&env);

    let token_id = mint_token(&env, &mint_client, &owner, 306);

    // Transferring to oneself is rejected.
    let result = client.try_transfer(&owner, &owner, &owner, &token_id);
    assert_eq!(result, Err(Ok(ContractError::SelfTransfer)));

    assert_eq!(mint_client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_invalid_batch() {
    let (env, client, mint_client) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    let token_1 = mint_token(&env, &mint_client, &from, 307);

    // Empty batch is invalid.
    let empty_requests: Vec<TransferRequest> = Vec::new(&env);
    let result = client.try_batch_transfer(&from, &empty_requests);
    assert_eq!(result, Err(Ok(ContractError::InvalidBatch)));

    // Batch containing a duplicate token id is invalid.
    let mut duplicate_requests = Vec::new(&env);
    duplicate_requests.push_back(TransferRequest {
        from: from.clone(),
        to: to.clone(),
        token_id: token_1,
        timestamp: 0,
        memo: None,
    });
    duplicate_requests.push_back(TransferRequest {
        from: from.clone(),
        to: to.clone(),
        token_id: token_1,
        timestamp: 0,
        memo: None,
    });

    let result = client.try_batch_transfer(&from, &duplicate_requests);
    assert_eq!(result, Err(Ok(ContractError::InvalidBatch)));

    // Ownership must remain unchanged after failed batches.
    assert_eq!(mint_client.owner_of(&token_1), from);
}
