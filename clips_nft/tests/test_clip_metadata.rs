#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use clips_nft::clip_metadata::ClipMetadata;

#[test]
fn test_clip_metadata_fields() {
    let env = Env::default();
    let creator: Address = Address::generate(&env);

    let metadata = ClipMetadata {
        title: String::from_str(&env, "My Clip"),
        description: String::from_str(&env, "A great clip"),
        thumbnail: String::from_str(&env, "ipfs://QmThumb"),
        ipfs_uri: String::from_str(&env, "ipfs://QmContent"),
        creator: creator.clone(),
        created_at: 1_700_000_000u64,
    };

    assert_eq!(metadata.title, String::from_str(&env, "My Clip"));
    assert_eq!(metadata.description, String::from_str(&env, "A great clip"));
    assert_eq!(metadata.thumbnail, String::from_str(&env, "ipfs://QmThumb"));
    assert_eq!(metadata.ipfs_uri, String::from_str(&env, "ipfs://QmContent"));
    assert_eq!(metadata.creator, creator);
    assert_eq!(metadata.created_at, 1_700_000_000u64);
}
