#![cfg(test)]

use clips_nft::{Error, ClipsNftContract, ClipsNftContractClient};
use clips_nft::config::Config;
use soroban_sdk::{testutils::Address as _, Address, Env};

mod test_helpers;
use test_helpers::setup;

#[test]
fn test_config_guard_authorized() {
    let ctx = setup();
    let env = ctx.env;

    let valid_config = Config {
        owner: ctx.admin.clone(),
        version: 1,
        platform_fee_bps: 100,
        default_royalty_bps: 500,
        paused: false,
        max_batch_mint_size: 50,
        max_batch_transfer_size: 50,
        max_collection_size: 10_000,
    };

    assert_eq!(
        ctx.client.try_set_config(&ctx.admin, &valid_config),
        Ok(Ok(()))
    );
}

#[test]
fn test_config_guard_unauthorized() {
    let ctx = setup();
    let intruder = Address::generate(ctx.env);

    let valid_config = Config {
        owner: ctx.admin.clone(),
        version: 1,
        platform_fee_bps: 100,
        default_royalty_bps: 500,
        paused: false,
        max_batch_mint_size: 50,
        max_batch_transfer_size: 50,
        max_collection_size: 10_000,
    };

    assert_eq!(
        ctx.client.try_set_config(&intruder, &valid_config),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
}

#[test]
fn test_config_guard_invalid_platform_fee() {
    let ctx = setup();
    
    let mut config = ctx.client.get_config().unwrap();
    // Over max platform fee (1000 = 10%)
    config.platform_fee_bps = 2000;

    assert_eq!(
        ctx.client.try_set_config(&ctx.admin, &config),
        Err(Ok(Error::InvalidBasisPoints))
    );
}

#[test]
fn test_config_guard_invalid_royalty() {
    let ctx = setup();
    
    let mut config = ctx.client.get_config().unwrap();
    // Over max royalty (10_000 = 100%)
    config.default_royalty_bps = 11_000;

    assert_eq!(
        ctx.client.try_set_config(&ctx.admin, &config),
        Err(Ok(Error::InvalidBasisPoints))
    );
}

#[test]
fn test_config_guard_invalid_combined_deduction() {
    let ctx = setup();
    
    let mut config = ctx.client.get_config().unwrap();
    // Combined > 10_000
    config.default_royalty_bps = 9_500;
    config.platform_fee_bps = 1_000;

    assert_eq!(
        ctx.client.try_set_config(&ctx.admin, &config),
        Err(Ok(Error::TotalDeductionsExceedSalePrice))
    );
}

#[test]
fn test_config_guard_invalid_batch_sizes() {
    let ctx = setup();
    
    let mut config = ctx.client.get_config().unwrap();
    config.max_batch_mint_size = 0;

    assert_eq!(
        ctx.client.try_set_config(&ctx.admin, &config),
        Err(Ok(Error::InvalidConfig))
    );

    let mut config2 = ctx.client.get_config().unwrap();
    config2.max_batch_transfer_size = 101; // typically max is 100

    assert_eq!(
        ctx.client.try_set_config(&ctx.admin, &config2),
        Err(Ok(Error::InvalidConfig))
    );
}
