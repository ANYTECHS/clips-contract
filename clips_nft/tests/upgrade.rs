#![cfg(test)]

mod test_helpers;

use clips_nft::{ClipsNftContract, ClipsNftContractClient, Royalty, RoyaltyRecipient};
use soroban_sdk::{testutils::{Address as _, BytesN as _}, Address, BytesN, Env, String, Vec};
use test_helpers::sign_mint;

#[test]
fn test_upgrade_preserves_existing_nft_and_royalty_state() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let contract_id = env.register(ClipsNftContract, ());
    let client = ClipsNftContractClient::new(&env, &contract_id);

    client.init(&admin);

    let sk_bytes = BytesN::<32>::random(&env).to_array();
    let signer_keypair = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pubkey = BytesN::from_array(&env, &signer_keypair.verifying_key().to_bytes());
    client.set_signer(&admin, &pubkey);

    let clip_id = 42u32;
    let metadata_uri = String::from_str(&env, "ipfs://QmUpgradeMigrationTest");
    let signature = sign_mint(&env, &signer_keypair, &owner, clip_id, &metadata_uri);

    let mut recipients = Vec::new(&env);
    recipients.push_back(RoyaltyRecipient {
        recipient: owner.clone(),
        basis_points: 700,
    });
    let royalty = Royalty { recipients, asset_address: None };

    let token_id = client.mint(&owner, &clip_id, &metadata_uri, &None, &None, &royalty, &false, &signature);
    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.owner_of(&token_id), owner.clone());

    let old_royalty = client.get_royalty(&token_id);
    assert_eq!(old_royalty.recipients.len(), 2);

    let new_wasm_hash = BytesN::from_array(&env, &[0xAB; 32]);
    client.upgrade(&admin, &new_wasm_hash).expect("upgrade should succeed");

    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.owner_of(&token_id), owner);

    let royalty_after = client.get_royalty(&token_id);
    assert_eq!(royalty_after.recipients.len(), 2);
    assert_eq!(royalty_after.recipients.get(0).unwrap().basis_points, 700);

    assert_eq!(client.version(), 1);
}
