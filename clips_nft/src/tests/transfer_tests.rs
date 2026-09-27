#![cfg(test)]

use crate::{ClipsNftContract, ClipsNftContractClient};
use crate::transfer_request::{BatchTransferRequest, TransferRequest};
use crate::types::TokenId;
use soroban_sdk::{testutils::{Address as _, Events}, Address, Env, Vec};

// We will use the AtomicMintContract to mint, but we must use ClipsNftContract to transfer.
use crate::{AtomicMintContract, AtomicMintContractClient};
use crate::atomic_mint::MintParams;
use crate::signature_replay_storage::hash_signature;
use soroban_sdk::{BytesN, String};
use crate::types::Royalty;

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
    let sig = BytesN::<64>::random(env);
    let params = MintParams {
        owner: owner.clone(),
        clip_id,
        metadata_uri: String::from_str(env, "ipfs://test"),
        royalty: Royalty {
            recipient: owner.clone(),
            basis_points: 0,
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
    
    // Execute transfer
    let req = TransferRequest {
        from: from.clone(),
        to: to.clone(),
        token_id,
        timestamp: None,
        memo: None,
    };
    client.transfer(&from, &req);

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
        timestamp: None,
        memo: None,
    });
    requests.push_back(TransferRequest {
        from: from.clone(),
        to: to.clone(),
        token_id: token_2,
        timestamp: None,
        memo: None,
    });

    let batch = BatchTransferRequest { requests };
    client.batch_transfer(&from, &batch);

    assert_eq!(mint_client.owner_of(&token_1), to);
    assert_eq!(mint_client.owner_of(&token_2), to);
    assert_eq!(mint_client.tokens_of_owner(&from).len(), 0);
    assert_eq!(mint_client.tokens_of_owner(&to).len(), 2);
}
