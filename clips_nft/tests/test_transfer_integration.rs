#![cfg(test)]

use clips_nft::{ClipsNftClient, Error};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(env: &Env) -> (ClipsNftClient<'_>, Address, Address) {
    let contract_id = env.register_contract(None, clips_nft::ClipsNft);
    let client = ClipsNftClient::new(env, &contract_id);

    let owner = Address::generate(env);
    let non_owner = Address::generate(env);

    client.initialize(&owner);

    (client, owner, non_owner)
}

#[test]
fn test_non_owner_cannot_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, owner, non_owner) = setup(&env);

    let token_id = client.mint(&owner);
    let recipient = Address::generate(&env);

    let result = client.try_transfer(&non_owner, &token_id, &recipient);
    assert_eq!(result, Err(Ok(Error::UnauthorizedOwner)));

    // Ownership should remain unchanged.
    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_non_owner_cannot_list_for_sale() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, owner, non_owner) = setup(&env);

    let token_id = client.mint(&owner);
    let price = 1000_i128;

    let result = client.try_list_for_sale(&non_owner, &token_id, &price);
    assert_eq!(result, Err(Ok(Error::UnauthorizedOwner)));

    // No listing should have been created.
    assert!(client.get_listing(&token_id).is_none());
}

#[test]
fn test_non_owner_cannot_accept_offer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, owner, non_owner) = setup(&env);

    let token_id = client.mint(&owner);
    let offerer = Address::generate(&env);
    let offer_amount = 500_i128;

    // Owner lists the token so an offer can be made against it.
    client.list_for_sale(&owner, &token_id, &1000_i128);

    // A third party makes an offer.
    client.make_offer(&offerer, &token_id, &offer_amount);

    // The non-owner attempts to accept the offer on behalf of the owner.
    let result = client.try_accept_offer(&non_owner, &token_id, &offerer);
    assert_eq!(result, Err(Ok(Error::UnauthorizedOwner)));

    // Ownership should remain unchanged.
    assert_eq!(client.owner_of(&token_id), owner);
}