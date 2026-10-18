#![cfg(test)]

use clips_nft::{Error, ClipsNftContract, ClipsNftContractClient};
use clips_nft::error_catalog::TokenNotFoundError;
use soroban_sdk::{testutils::Address as _, Address, Env};

mod test_helpers;
use test_helpers::setup;

#[test]
fn test_nft_existence_guard_success() {
    let ctx = setup();
    let minter = Address::generate(ctx.env);

    // Mint a token
    let token_id = ctx.client.mint(&minter);

    // Any operation should succeed because token exists
    // Let's try fetching the metadata
    let metadata_result = ctx.client.try_get_metadata(&token_id);
    assert!(metadata_result.is_ok());
}

#[test]
fn test_nft_existence_guard_failure() {
    let ctx = setup();
    let minter = Address::generate(ctx.env);
    
    let non_existent_token_id = 999;

    // Trying to transfer a nonexistent token
    let res = ctx.client.try_transfer(
        &minter,
        &minter,
        &Address::generate(ctx.env),
        &non_existent_token_id,
    );

    // The guard should intercept and return TokenNotFound error
    // In the error_catalog, TokenNotFoundError::CODE is 230
    assert_eq!(
        res,
        Err(Ok(Error::TokenNotFound))
    );
}
