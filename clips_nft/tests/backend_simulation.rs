#![cfg(test)]

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Royalty, RoyaltyRecipient};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _},
    Address, Bytes, BytesN, Env, String, Vec, xdr::ToXdr,
    token,
};

/// Mock Backend Simulation
struct MockBackend {
    signer_secret: ed25519_dalek::SigningKey,
}

impl MockBackend {
    fn new(env: &Env) -> Self {
        let sk_bytes = soroban_sdk::BytesN::<32>::random(env).to_array();
        let signer_secret = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        Self { signer_secret }
    }

    fn public_key(&self, env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &self.signer_secret.verifying_key().to_bytes())
    }

    /// Simulates uploading metadata to IPFS/Arweave and returning the URI.
    fn upload_metadata(&self, env: &Env, clip_id: u32) -> String {
        // In a real scenario, this would involve hashing the content and uploading it.
        // Here we just return a deterministic URI based on clip_id.
        String::from_str(env, &format!("ipfs://QmClip{}", clip_id))
    }

    /// Simulates the backend signing a mint payload.
    fn sign_mint(
        &self,
        env: &Env,
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
        let sig = self.signer_secret.sign(&message.to_array());
        BytesN::from_array(env, &sig.to_bytes())
    }
}

fn setup_test(env: &Env) -> (ClipsNftContractClient<'_>, Address, MockBackend) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(env, &contract_id);
    client.init(&admin);

    let backend = MockBackend::new(env);
    client.set_signer(&admin, &backend.public_key(env));

    (client, admin, backend)
}

#[test]
fn test_mint_after_metadata_upload() {
    let env = Env::default();
    let (client, _admin, backend) = setup_test(&env);
    let user = Address::generate(&env);

    // 1. Backend "uploads" metadata
    let clip_id = 101;
    let metadata_uri = backend.upload_metadata(&env, clip_id);

    // 2. Backend signs the mint request for the user
    let signature = backend.sign_mint(&env, &user, clip_id, &metadata_uri);

    // 3. User calls mint
    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: user.clone(),
        basis_points: 1000, // 10%
    });
    let royalty = Royalty {
        recipients,
        asset_address: None, // XLM
    };

    let token_id = client.mint(&user, &clip_id, &metadata_uri, &royalty, &false, &signature);

    // Verification
    assert_eq!(token_id, 1);
    assert_eq!(client.owner_of(&token_id), user);
    assert_eq!(client.token_uri(&token_id), metadata_uri);
    assert_eq!(client.clip_token_id(&clip_id), 1);
}

#[test]
fn test_royalty_on_secondary_sale() {
    let env = Env::default();
    let (client, admin, backend) = setup_test(&env);
    let creator = Address::generate(&env);
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    // Setup mock payment token (USDC-like)
    let token_admin = Address::generate(&env);
    let token_address = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let token_client = token::TokenClient::new(&env, &token_address);
    let token_admin_client = token::StellarAssetClient::new(&env, &token_address);

    // Mint some tokens to buyer1 for the sale
    let sale_price = 1000_i128;
    token_admin_client.mint(&buyer1, &sale_price);

    // 1. Mint NFT for creator with 5% royalty (plus default 1% for platform)
    let clip_id = 202;
    let metadata_uri = backend.upload_metadata(&env, clip_id);
    let signature = backend.sign_mint(&env, &creator, clip_id, &metadata_uri);

    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: creator.clone(),
        basis_points: 500, // 5%
    });
    let royalty = Royalty {
        recipients,
        asset_address: Some(token_address.clone()),
    };

    let token_id = client.mint(&creator, &clip_id, &metadata_uri, &royalty, &false, &signature);

    // 2. Primary sale: creator -> buyer1 (handled off-chain or via another contract, here we just transfer)
    client.transfer(&creator, &buyer1, &token_id, &sale_price);
    assert_eq!(client.owner_of(&token_id), buyer1);

    // 3. Secondary sale: buyer1 -> buyer2
    // Before transferring, buyer1 pays royalties
    client.pay_royalty(&buyer1, &token_id, &sale_price);

    // Verify royalty distribution:
    // 5% to creator = 50
    // 1% to platform (admin) = 10
    assert_eq!(token_client.balance(&creator), 50);
    assert_eq!(token_client.balance(&admin), 10);
    assert_eq!(token_client.balance(&buyer1), 1000 - 60);

    // Complete the transfer
    client.transfer(&buyer1, &buyer2, &token_id, &sale_price);
    assert_eq!(client.owner_of(&token_id), buyer2);
}

#[test]
fn test_error_cases() {
    let env = Env::default();
    let (client, admin, backend) = setup_test(&env);
    let user = Address::generate(&env);

    let clip_id = 303;
    let metadata_uri = backend.upload_metadata(&env, clip_id);
    let royalty = Royalty {
        recipients: Vec::new(&env),
        asset_address: None,
    };

    // Case 1: Invalid Signature (tampered clip_id)
    let signature = backend.sign_mint(&env, &user, clip_id, &metadata_uri);
    let result = client.try_mint(&user, &(clip_id + 1), &metadata_uri, &royalty, &false, &signature);
    assert!(result.is_err());
    // We can check the exact error if we want, but is_err is usually enough for integration tests
    
    // Case 2: Double Minting the same clip_id
    client.mint(&user, &clip_id, &metadata_uri, &royalty, &false, &signature);
    let result = client.try_mint(&user, &clip_id, &metadata_uri, &royalty, &false, &signature);
    assert!(result.is_err());

    // Case 3: Unauthorized set_signer
    let malicious = Address::generate(&env);
    let result = client.try_set_signer(&malicious, &backend.public_key(&env));
    assert!(result.is_err());

    // Case 4: Transfer while paused
    let token_id = 1;
    client.pause(&admin);
    let result = client.try_transfer(&user, &malicious, &token_id, &1_000_000);
    assert!(result.is_err());
}
