#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use clips_nft::royalty_config::RoyaltyConfig;

#[test]
fn test_royalty_config_valid() {
    let env = Env::default();
    let recipient: Address = Address::generate(&env);
    let config = RoyaltyConfig { recipient, royalty_bps: 500 };
    assert!(config.validate().is_ok());
}

#[test]
fn test_royalty_config_max_bps_valid() {
    let env = Env::default();
    let recipient: Address = Address::generate(&env);
    let config = RoyaltyConfig { recipient, royalty_bps: 10_000 };
    assert!(config.validate().is_ok());
}

#[test]
fn test_royalty_config_exceeds_max_fails() {
    let env = Env::default();
    let recipient: Address = Address::generate(&env);
    let config = RoyaltyConfig { recipient, royalty_bps: 10_001 };
    assert!(config.validate().is_err());
}
