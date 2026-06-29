#![cfg(test)]

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Royalty, RoyaltyRecipient};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger as _},
    Address, Bytes, BytesN, Env, String, Vec, xdr::ToXdr,
};

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
fn test_set_name_and_symbol() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    client.init(&admin);

    // Default values
    assert_eq!(client.name(), String::from_str(&env, "ClipCash Clips"));
    assert_eq!(client.symbol(), String::from_str(&env, "CLIP"));

    // Set new name and symbol
    let new_name = String::from_str(&env, "Awesome Clips");
    let new_symbol = String::from_str(&env, "AWCLIP");
    client.set_name(&admin, &new_name);
    client.set_symbol(&admin, &new_symbol);

    assert_eq!(client.name(), new_name);
    assert_eq!(client.symbol(), new_symbol);
}

#[test]
fn test_blacklist_clip() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    client.init(&admin);

    // Setup signer
    let sk_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
    let signer_keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pubkey = BytesN::from_array(&env, &signer_keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey);

    let clip_id = 42u32;
    let metadata_uri = String::from_str(&env, "ipfs://test123");
    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: user.clone(),
        basis_points: 100,
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };
    let signature = sign_mint(&env, &signer_keypair, &user, clip_id, &metadata_uri);

    // Mint before blacklist should work
    let token_id = client.mint(&user, &clip_id, &metadata_uri, &None, &None, &royalty, &false, &None, &signature);
    assert_eq!(token_id, 1);

    // Blacklist the clip
    client.blacklist_clip(&admin, &(clip_id + 1));

    // Try to mint blacklisted clip
    let metadata_uri2 = String::from_str(&env, "ipfs://test456");
    let signature2 = sign_mint(&env, &signer_keypair, &user, clip_id + 1, &metadata_uri2);
    let mut recipients2 = Vec::new(&env);
    recipients2.push_back(RoyaltyRecipient {
        recipient: user.clone(),
        basis_points: 100,
    });
    let royalty2 = Royalty {
        recipients: recipients2,
        asset_address: None,
    };

    let result = client.try_mint(&user, &(clip_id + 1), &metadata_uri2, &None, &None, &royalty2, &false, &None, &signature2);
    assert!(result.is_err());
}

#[test]
fn test_get_clip_id() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    client.init(&admin);

    // Setup signer
    let sk_bytes = soroban_sdk::BytesN::<32>::random(&env).to_array();
    let signer_keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pubkey = BytesN::from_array(&env, &signer_keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey);

    let clip_id = 9876u32;
    let metadata_uri = String::from_str(&env, "ipfs://test456");
    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: user.clone(),
        basis_points: 100,
    });
    let royalty = Royalty {
        recipients,
        asset_address: None,
    };
    let signature = sign_mint(&env, &signer_keypair, &user, clip_id, &metadata_uri);

    let token_id = client.mint(&user, &clip_id, &metadata_uri, &None, &None, &royalty, &false, &None, &signature);
    assert_eq!(client.get_clip_id(&token_id), clip_id);
}
