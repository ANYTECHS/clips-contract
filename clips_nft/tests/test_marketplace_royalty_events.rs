#![cfg(test)]

use clips_nft::{
    mint_service::execute_mint,
    types::{TokenId, Royalty, RoyaltyRecipient},
    MintRequest, ClipsNftContract, ListingRequest,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events, Ledger, LedgerInfo},
    Address, Env, String, Symbol, vec, TryFromVal,
};

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let key = symbol_short!("bal");
        let mut balances: soroban_sdk::Map<Address, i128> = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| soroban_sdk::Map::new(&env));
        let cur = balances.get(to.clone()).unwrap_or(0);
        balances.set(to.clone(), cur + amount);
        env.storage().instance().set(&key, &balances);
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        // mock transfer
    }
}

fn setup_env_and_mint(env: &Env, contract_id: &Address, bps: u32, payment_asset: &Address) -> (Address, Address, TokenId) {
    let owner = Address::generate(env);
    let recipient = Address::generate(env);

    let royalty_recipient = RoyaltyRecipient {
        recipient: recipient.clone(),
        basis_points: bps,
    };

    let req = MintRequest {
        clip_id: 1,
        owner: owner.clone(),
        creator: owner.clone(),
        creator_address: Some(owner.clone()),
        creator_display_name: Some(String::from_str(env, "Creator")),
        metadata_uri: String::from_str(env, "ipfs://QmClip1"),
        thumbnail_uri: None,
        preview_video_uri: None,
        royalty_info: Royalty {
            recipients: vec![env, royalty_recipient],
            asset_address: Some(payment_asset.clone()),
        },
    };

    let result = env.as_contract(contract_id, || {
        // execute_mint sets TokenData but not TokenOwner directly for validation in tests
        clips_nft::token_owner_storage::assign_owner(env, 1, &owner, 1).unwrap();
        clips_nft::payment_currency::add_currency(env, payment_asset.clone()).unwrap();
        execute_mint(env, req).expect("mint failed")
    });
    (owner, recipient, result.token_id)
}

fn has_event(env: &Env, name: &str) -> bool {
    let sym = Symbol::new(env, name);
    for evt in env.events().all().events() {
        if let soroban_sdk::xdr::ContractEventBody::V0(v0) = &evt.body {
            if v0.topics.len() > 0 {
                if let Ok(topic_sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                    if topic_sym == sym {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn count_events(env: &Env, name: &str) -> usize {
    let mut count = 0;
    let sym = Symbol::new(env, name);
    for evt in env.events().all().events() {
        if let soroban_sdk::xdr::ContractEventBody::V0(v0) = &evt.body {
            if v0.topics.len() > 0 {
                if let Ok(topic_sym) = Symbol::try_from_val(env, &v0.topics[0]) {
                    if topic_sym == sym {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

#[test]
fn test_marketplace_events() {
    let env = Env::default();
    
    // 1. Setup Mock Token
    let token_id_addr = env.register(MockToken, ());
    let token_client = MockTokenClient::new(&env, &token_id_addr);

    let contract_id = env.register(ClipsNftContract, ());
    let client = clips_nft::ClipsNftContractClient::new(&env, &contract_id);
    client.init(&Address::generate(&env));

    let (seller, _recipient, token_id) = setup_env_and_mint(&env, &contract_id, 500, &token_id_addr);
    let buyer = Address::generate(&env);
    let price = 1000;
    
    // Give buyer money
    token_client.mint(&buyer, &20000);
    
    env.mock_all_auths();

    let listing_req = ListingRequest {
        listing_id: 0,
        token_id,
        price,
        payment_asset: token_id_addr.clone(),
        expiration: 0,
        seller: seller.clone(),
    };

    // 2. Test Listing
    client.create_listing(&listing_req);
    
    assert!(has_event(&env, "nft_list"), "Expected NftListedEvent on list");

    // 3. Test Update Listing (emits NftListedEvent)
    client.update_listing(&seller, &token_id, &2000, &0);
    assert!(has_event(&env, "nft_list"), "Expected NftListedEvent on update");
    // 4. Test Cancellation
    client.cancel_listing(&seller, &token_id);
    assert!(has_event(&env, "lst_cancl"), "Expected ListingCancelledEvent");

    // Re-list for sale
    client.create_listing(&listing_req);

    // 5. Test Sale and Royalty Payment
    client.buy_listing(&buyer, &token_id, &token_id_addr, &price);
    
    assert!(has_event(&env, "nft_sold"), "Expected NftSoldEvent");
    assert!(has_event(&env, "ryl_paid"), "Expected RoyaltyPaidEvent on sale");

    // Buyer is now owner, they can accept offers. Let's make an offer.
    // 6. Test Offer Creation
    let offerer = Address::generate(&env);
    token_client.mint(&offerer, &5000);
    
    client.make_offer(&offerer, &token_id, &1500, &token_id_addr, &0);
    
    assert!(has_event(&env, "offr_crea"), "Expected OfferCreatedEvent");

    // 7. Test Offer Acceptance
    client.accept_offer(&buyer, &token_id);
    
    assert!(has_event(&env, "ofr_accpt"), "Expected OfferAcceptedEvent");
    assert!(has_event(&env, "ryl_paid"), "Expected RoyaltyPaidEvent on offer acceptance");
}
