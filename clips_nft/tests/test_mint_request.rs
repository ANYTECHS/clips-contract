#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

use clips_nft::mint_request::MintRequest;
use clips_nft::{Royalty, RoyaltyRecipient};

#[test]
fn test_mint_request_fields() {
    let env = Env::default();
    let owner: Address = Address::generate(&env);
    let recipient: Address = Address::generate(&env);

    let royalty = Royalty {
        recipients: Vec::from_array(
            &env,
            [RoyaltyRecipient { recipient: recipient.clone(), basis_points: 500 }],
        ),
        asset_address: None,
    };

    let req = MintRequest {
        clip_id: 42u32,
        owner: owner.clone(),
        metadata_uri: String::from_str(&env, "ipfs://QmXyz"),
        royalty_info: royalty.clone(),
    };

    assert_eq!(req.clip_id, 42u32);
    assert_eq!(req.owner, owner);
    assert_eq!(req.metadata_uri, String::from_str(&env, "ipfs://QmXyz"));
    assert_eq!(req.royalty_info.recipients.len(), 1);
}
