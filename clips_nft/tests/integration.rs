#![cfg(test)]

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Royalty, RoyaltyRecipient};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, Bytes, BytesN, Env, String, Vec, xdr::ToXdr,
};

/// Helper to sign a mint payload.
/// This simulates the backend signing process.
fn sign_mint(
    env: &Env,
    signer_secret: &ed25519_dalek::SigningKey,
    owner: &Address,
    clip_id: u32,
    metadata_uri: &String,
) -> BytesN<64> {
    let owner_hash: BytesN<32> = env.crypto().sha256(&owner.clone().to_xdr(env)).into();
    let uri_hash: BytesN<32> = env.crypto().sha256(&Bytes::from(metadata_uri.to_xdr(env))).into();

    let mut preimage = Bytes::new(env);
    preimage.extend_from_array(&clip_id.to_le_bytes());
    preimage.append(&Bytes::from(owner_hash));
    preimage.append(&Bytes::from(uri_hash));

    let message: BytesN<32> = env.crypto().sha256(&preimage).into();
    use ed25519_dalek::Signer as _;
    let sig = signer_secret.sign(&message.to_array());
    BytesN::from_array(env, &sig.to_bytes())
}

#[test]
fn test_integration_wallet_simulation_mint_and_royalty() {
    let env = Env::default();
    
    // 1. Simulate Wallet Connection (Freighter)
    // In Soroban tests, we simulate a wallet by generating an Address.
    // mock_all_auths() allows us to simulate the user approving the transaction in their wallet.
    env.mock_all_auths();
    
    let admin = Address::generate(&env);
    let user_wallet = Address::generate(&env);
    let platform_recipient = admin.clone(); // Admin acts as platform recipient in this test
    
    // Register the contract
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    
    // 2. Initialize the contract
    client.init(&admin);
    
    // 3. Setup backend signer (simulated backend registration)
    let sk_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
    let signer_keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pubkey = BytesN::from_array(&env, &signer_keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey);
    
    // 4. End-to-End Mint Flow
    let clip_id = 12345u32;
    let metadata_uri = String::from_str(&env, "ipfs://QmVideoClip12345");
    
    // Prepare royalty info (5% for creator, platform gets 1% default)
    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: user_wallet.clone(),
        basis_points: 500, // 5%
    });
    let royalty = Royalty {
        recipients,
        asset_address: None, // XLM
    };
    
    // Simulate backend signing the request
    let signature = sign_mint(&env, &signer_keypair, &user_wallet, clip_id, &metadata_uri);
    
    // User "connects" wallet and calls mint
    // The call to `mint` will require user_wallet's authorization, which is provided by `mock_all_auths()`.
    let token_id = client.mint(
        &user_wallet,
        &clip_id,
        &metadata_uri,
        &royalty,
        &false,
        &signature
    );
    
    // Verify Mint Result
    assert_eq!(token_id, 1);
    assert_eq!(client.owner_of(&token_id), user_wallet);
    assert_eq!(client.token_uri(&token_id), metadata_uri);
    assert_eq!(client.total_supply(), 1);
    
    // 5. Test Royalty Flow
    // Simulate a sale price of 1000 XLM (in stroops or arbitrary units)
    let sale_price = 1000_i128;
    let royalty_info = client.royalty_info(&token_id, &sale_price);
    
    // Total royalty should be 5% (creator) + 1% (platform) = 6%
    // 1000 * 0.06 = 60
    assert_eq!(royalty_info.royalty_amount, 60);
    assert_eq!(royalty_info.receiver, user_wallet); // First recipient is the creator
    assert_eq!(royalty_info.asset_address, None);
    
    // 6. Verify full royalty configuration
    let stored_royalty = client.get_royalty(&token_id);
    assert_eq!(stored_royalty.recipients.len(), 2); // Creator + Platform
    
    // First recipient should be creator (500 bps)
    let creator_split = stored_royalty.recipients.get(0).unwrap();
    assert_eq!(creator_split.recipient, user_wallet);
    assert_eq!(creator_split.basis_points, 500);
    
    // Second recipient should be platform (100 bps = 1%)
    let platform_split = stored_royalty.recipients.get(1).unwrap();
    assert_eq!(platform_split.recipient, platform_recipient);
    assert_eq!(platform_split.basis_points, 100);

    // 7. End-to-End Transfer Flow (Simulating a sale/gift via wallet)
    let new_owner = Address::generate(&env);
    
    // User wallet authorizes the transfer
    client.transfer(&user_wallet, &new_owner, &token_id);
    
    // Verify transfer
    assert_eq!(client.owner_of(&token_id), new_owner);
    
    // 8. Test "Is Paused" flow (Wallet should respect pause)
    client.pause(&admin);
    assert!(client.is_paused());
    
    // Attempting to transfer while paused should fail (mock_all_auths still applies, but contract logic blocks it)
    let result = client.try_transfer(&new_owner, &user_wallet, &token_id);
    assert!(result.is_err());
    
    // Unpause
    client.unpause(&admin);
    assert!(!client.is_paused());
    
    // Transfer should work now
    client.transfer(&new_owner, &user_wallet, &token_id);
    assert_eq!(client.owner_of(&token_id), user_wallet);
}

#[test]
fn test_approval_and_approval_for_all_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let operator = Address::generate(&env);

    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    client.init(&admin);

    let sk_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
    let signer_keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pubkey = BytesN::from_array(&env, &signer_keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey).unwrap();

    let clip_id = 9001u32;
    let metadata_uri = String::from_str(&env, "ipfs://QmApproval9001");
    let signature = sign_mint(&env, &signer_keypair, &owner, clip_id, &metadata_uri);
    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: owner.clone(),
        basis_points: 500,
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };
    let token_id = client
        .mint(&owner, &clip_id, &metadata_uri, &royalty, &false, &signature)
        .unwrap();

    client
        .set_approval_for_all(&owner, &operator, &true)
        .unwrap();
    assert!(client.is_approved_for_all(&owner, &operator));

    client
        .approve(&owner, &Some(operator.clone()), &token_id)
        .unwrap();
    assert_eq!(client.get_approved(&token_id), Some(operator.clone()));
}

#[test]
fn test_name_and_symbol_configurable_by_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    client.init(&admin);

    assert_eq!(client.name(), String::from_str(&env, "ClipCash Clips"));
    assert_eq!(client.symbol(), String::from_str(&env, "CLIP"));

    client.set_name(&admin, &String::from_str(&env, "My Clips")).unwrap();
    client
        .set_symbol(&admin, &String::from_str(&env, "MCLIP"))
        .unwrap();

    assert_eq!(client.name(), String::from_str(&env, "My Clips"));
    assert_eq!(client.symbol(), String::from_str(&env, "MCLIP"));

    assert!(client
        .try_set_name(&non_admin, &String::from_str(&env, "Nope"))
        .is_err());
}

#[test]
fn test_batch_mint_enforces_gas_safe_limit() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);

    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);
    client.init(&admin);

    let sk_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
    let signer_keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pubkey = BytesN::from_array(&env, &signer_keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey).unwrap();

    let mut clip_ids = Vec::new(&env);
    let mut metadata_uris = Vec::new(&env);
    let mut signatures = Vec::new(&env);
    for i in 0..26u32 {
        let clip_id = 10_000 + i;
        let metadata_uri = String::from_str(&env, &format!("ipfs://QmBatch{}", clip_id));
        let signature = sign_mint(&env, &signer_keypair, &owner, clip_id, &metadata_uri);
        clip_ids.push_back(clip_id);
        metadata_uris.push_back(metadata_uri);
        signatures.push_back(signature);
    }

    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: owner.clone(),
        basis_points: 500,
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };

    assert!(client
        .try_batch_mint(&owner, &clip_ids, &metadata_uris, &royalty, &false, &signatures)
        .is_err());
}
