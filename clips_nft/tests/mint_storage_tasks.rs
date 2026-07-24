//! Integration tests for mint storage tasks (signature replay, owner, wallet index, atomic mint).

use clips_nft::{
    hash_signature, AtomicMintContract, AtomicMintContractClient, MintParams, Royalty,
};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, BytesN, Env, String,
};

fn setup() -> (Env, Address, AtomicMintContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(AtomicMintContract, ());
    let client = AtomicMintContractClient::new(&env, &contract_id);
    client.init(&admin);
    (env, contract_id, client)
}

fn mint_params(env: &Env, owner: &Address, clip_id: u32, sig: &BytesN<64>) -> MintParams {
    MintParams {
        owner: owner.clone(),
        clip_id,
        metadata_uri: String::from_str(env, &format!("ipfs://clip-{}", clip_id)),
        royalty: Royalty {
            recipient: owner.clone(),
            basis_points: 750,
            asset_address: None,
        },
        signature_hash: hash_signature(env, sig),
    }
}

// ── Task 1: Signature replay protection ─────────────────────────────────────

#[test]
fn signature_replay_storage_rejects_duplicate_hash() {
    let (env, _contract, client) = setup();
    let owner = Address::generate(&env);
    let sig = BytesN::<64>::random(&env);
    let hash = hash_signature(&env, &sig);

    assert!(!client.signature_used(&hash));
    client.mint(&mint_params(&env, &owner, 1, &sig));
    assert!(client.signature_used(&hash));

    let sig2 = BytesN::<64>::random(&env);
    let mut replay = mint_params(&env, &owner, 2, &sig2);
    replay.signature_hash = hash;
    assert!(client.try_mint(&replay).is_err());
    assert_eq!(client.next_token_id(), 1);
}

// ── Task 2: Assign initial NFT owner ──────────────────────────────────────────

#[test]
fn mint_assigns_owner_from_request() {
    let (env, _contract, client) = setup();
    let owner = Address::generate(&env);
    let sig = BytesN::<64>::random(&env);

    let token_id = client.mint(&mint_params(&env, &owner, 10, &sig));
    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn mint_rejects_contract_address_as_owner() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(AtomicMintContract, ());
    let client = AtomicMintContractClient::new(&env, &contract_id);
    client.init(&admin);

    env.as_contract(&contract_id, || {
        let contract_addr = env.current_contract_address();
        let sig = BytesN::<64>::random(&env);
        let params = mint_params(&env, &contract_addr, 11, &sig);
        let result = client.try_mint(&params);
        assert!(result.is_err());
    });
}

// ── Task 3: Register token in owner index ─────────────────────────────────────

#[test]
fn mint_registers_token_in_wallet_index() {
    let (env, _contract, client) = setup();
    let owner = Address::generate(&env);

    let sig1 = BytesN::<64>::random(&env);
    let t0 = client.mint(&mint_params(&env, &owner, 20, &sig1));
    let sig2 = BytesN::<64>::random(&env);
    let t1 = client.mint(&mint_params(&env, &owner, 21, &sig2));

    let tokens = client.tokens_of_owner(&owner);
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens.get(0).unwrap(), t0);
    assert_eq!(tokens.get(1).unwrap(), t1);
}

#[test]
fn wallet_index_prevents_duplicate_token_entry() {
    let (env, _contract, client) = setup();
    let owner = Address::generate(&env);
    let sig = BytesN::<64>::random(&env);
    client.mint(&mint_params(&env, &owner, 30, &sig));
    assert_eq!(client.tokens_of_owner(&owner).len(), 1);
}

// ── Task 4: Prevent partial mint failures ─────────────────────────────────────

#[test]
fn failed_mint_leaves_no_orphan_token_record() {
    let (env, _contract, client) = setup();
    let owner = Address::generate(&env);

    let sig1 = BytesN::<64>::random(&env);
    client.mint(&mint_params(&env, &owner, 40, &sig1));

    let sig2 = BytesN::<64>::random(&env);
    let mut dup = mint_params(&env, &owner, 40, &sig2);
    dup.metadata_uri = String::from_str(&env, "ipfs://different-uri");

    assert!(client.try_mint(&dup).is_err());
    assert_eq!(client.next_token_id(), 1);
    assert!(!client.token_exists(&1));
}

#[test]
fn write_phase_failure_rolls_back_all_mint_storage() {
    let (env, contract_id, client) = setup();
    let owner = Address::generate(&env);

    env.as_contract(&contract_id, || {
        clips_nft::wallet_token_index::add_token_to_wallet(&env, &owner, 0).unwrap();
    });

    let sig = BytesN::<64>::random(&env);
    let params = mint_params(&env, &owner, 60, &sig);
    assert!(client.try_mint(&params).is_err());
    assert!(!client.token_exists(&0));
    assert_eq!(client.next_token_id(), 0);
    assert!(!client.signature_used(&params.signature_hash));
}

#[test]
fn failed_mint_does_not_consume_signature_hash() {
    let (env, _contract, client) = setup();
    let owner = Address::generate(&env);

    let sig1 = BytesN::<64>::random(&env);
    client.mint(&mint_params(&env, &owner, 50, &sig1));

    let sig2 = BytesN::<64>::random(&env);
    let hash2 = hash_signature(&env, &sig2);
    let mut dup = mint_params(&env, &owner, 50, &sig2);
    dup.metadata_uri = String::from_str(&env, "ipfs://retry-uri");
    assert!(client.try_mint(&dup).is_err());
    assert!(!client.signature_used(&hash2));
}
