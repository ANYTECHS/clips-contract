//! Comprehensive contract error test suite for ClipCash (#1403).
//!
//! Covers every error category across all modules:
//! initialization, configuration, minting, NFT ownership, transfers,
//! approvals, royalties, marketplace, payments, batch operations,
//! frozen tokens, unauthorized access, unsupported assets, invalid
//! parameters, contract pause state, duplicate operations, and storage
//! failures.

use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, String, Vec};

use crate::{
    errors::Error,
    tests::test_helpers::{self, setup_contract, setup_contract_with_admin},
    types::{Asset, BatchMintItem, RoyaltyInfo},
    ContractError,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_err<T: core::fmt::Debug>(
    result: Result<T, ContractError>,
    expected: Error,
) {
    match result {
        Ok(value) => panic!("expected error {:?}, got Ok({:?})", expected, value),
        Err(err) => {
            assert_eq!(err, expected, "unexpected error variant");
            assert_eq!(err as u32, expected as u32, "error code mismatch");
        }
    }
}

fn sample_asset(env: &Env) -> Asset {
    Asset::Native
}

fn sample_royalty(env: &Env) -> RoyaltyInfo {
    RoyaltyInfo {
        recipient: Address::generate(env),
        basis_points: 500,
    }
}

// ---------------------------------------------------------------------------
// Initialization errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_initialization_already_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_initialize(&Address::generate(&env));
    assert_err(result, Error::AlreadyInitialized);
}

#[test]
fn test_error_initialization_invalid_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let client = test_helpers::deploy_uninitialized(&env);

    // Zero/invalid admin address should be rejected.
    let zero = Address::from_string(&String::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ));
    let result = client.try_initialize(&zero);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_initialization_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let client = test_helpers::deploy_uninitialized(&env);

    let result = client.try_mint(&Address::generate(&env), &sample_asset(&env));
    assert_err(result, Error::NotInitialized);
}

// ---------------------------------------------------------------------------
// Configuration errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_configuration_invalid_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    // Fee above 100% (10_000 bps) is invalid.
    let result = client.try_set_marketplace_fee(&20_000);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_configuration_invalid_royalty_bps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_set_default_royalty(&10_001);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_configuration_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let attacker = Address::generate(&env);
    let result = client.try_set_marketplace_fee_as(&attacker, &100);
    assert_err(result, Error::Unauthorized);
}

// ---------------------------------------------------------------------------
// Minting errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_minting_unsupported_asset() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let unsupported = Asset::Token(Address::generate(&env));
    let result = client.try_mint(&Address::generate(&env), &unsupported);
    match result {
        Ok(_) => panic!("expected UnsupportedAsset error"),
        Err(err) => {
            assert_eq!(err, Error::UnsupportedAsset);
            // The asset must be carried in the error payload.
            let carried = client.get_last_unsupported_asset();
            assert_eq!(carried, unsupported, "asset not surfaced in error");
        }
    }
}

#[test]
fn test_error_minting_invalid_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let empty = String::from_str(&env, "");
    let result = client.try_mint_with_metadata(
        &Address::generate(&env),
        &sample_asset(&env),
        &empty,
    );
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_minting_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);
    client.pause();

    let result = client.try_mint(&Address::generate(&env), &sample_asset(&env));
    assert_err(result, Error::ContractPaused);
}

// ---------------------------------------------------------------------------
// NFT ownership errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_ownership_token_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_owner_of(&9999);
    assert_err(result, Error::TokenNotFound);
}

#[test]
fn test_error_ownership_not_owner() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let attacker = Address::generate(&env);

    let result = client.try_transfer(&attacker, &owner, &token_id);
    assert_err(result, Error::Unauthorized);
}

// ---------------------------------------------------------------------------
// Transfer errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_transfer_to_self() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));

    let result = client.try_transfer(&owner, &owner, &token_id);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_transfer_frozen_token() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    client.freeze(&token_id);

    let result = client.try_transfer(&owner, &Address::generate(&env), &token_id);
    assert_err(result, Error::TokenFrozen);
}

#[test]
fn test_error_transfer_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    client.pause();

    let result = client.try_transfer(&owner, &Address::generate(&env), &token_id);
    assert_err(result, Error::ContractPaused);
}

// ---------------------------------------------------------------------------
// Approval errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_approval_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let attacker = Address::generate(&env);

    let result = client.try_approve(&attacker, &Address::generate(&env), &token_id);
    assert_err(result, Error::Unauthorized);
}

#[test]
fn test_error_approval_token_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_approve(
        &Address::generate(&env),
        &Address::generate(&env),
        &9999,
    );
    assert_err(result, Error::TokenNotFound);
}

// ---------------------------------------------------------------------------
// Royalty errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_royalty_exceeds_max() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));

    let bad = RoyaltyInfo {
        recipient: Address::generate(&env),
        basis_points: 10_001,
    };
    let result = client.try_set_royalty(&owner, &token_id, &bad);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_royalty_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let attacker = Address::generate(&env);

    let result = client.try_set_royalty(&attacker, &token_id, &sample_royalty(&env));
    assert_err(result, Error::Unauthorized);
}

// ---------------------------------------------------------------------------
// Marketplace errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_marketplace_listing_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_buy(&Address::generate(&env), &9999);
    assert_err(result, Error::ListingNotFound);
}

#[test]
fn test_error_marketplace_unauthorized_listing() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let attacker = Address::generate(&env);

    let result = client.try_list(&attacker, &token_id, &100);
    assert_err(result, Error::Unauthorized);
}

#[test]
fn test_error_marketplace_invalid_price() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));

    let result = client.try_list(&owner, &token_id, &0);
    assert_err(result, Error::InvalidParameter);
}

// ---------------------------------------------------------------------------
// Payment errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_payment_insufficient() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let listing_id = client.list(&owner, &token_id, &1_000);

    let buyer = Address::generate(&env);
    let result = client.try_buy_with_payment(&buyer, &listing_id, &500);
    match result {
        Ok(_) => panic!("expected InsufficientPayment error"),
        Err(err) => {
            assert_eq!(err, Error::InsufficientPayment);
            // The expected amount must be surfaced in the error payload.
            let expected_amount = client.get_last_expected_payment();
            assert_eq!(expected_amount, 1_000, "expected amount not surfaced");
        }
    }
}

#[test]
fn test_error_payment_unsupported_asset() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let listing_id = client.list(&owner, &token_id, &1_000);

    let buyer = Address::generate(&env);
    let bad_asset = Asset::Token(Address::generate(&env));
    let result = client.try_buy_with_asset(&buyer, &listing_id, &bad_asset);
    match result {
        Ok(_) => panic!("expected UnsupportedAsset error"),
        Err(err) => {
            assert_eq!(err, Error::UnsupportedAsset);
            let carried = client.get_last_unsupported_asset();
            assert_eq!(carried, bad_asset, "asset not surfaced in error");
        }
    }
}

// ---------------------------------------------------------------------------
// Batch operation errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_batch_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let items: Vec<BatchMintItem> = Vec::new(&env);
    let result = client.try_batch_mint(&Address::generate(&env), &items);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_batch_too_large() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let mut items: Vec<BatchMintItem> = Vec::new(&env);
    for _ in 0..1_000 {
        items.push_back(BatchMintItem {
            asset: sample_asset(&env),
            metadata: String::from_str(&env, "meta"),
        });
    }
    let result = client.try_batch_mint(&Address::generate(&env), &items);
    assert_err(result, Error::BatchTooLarge);
}

#[test]
fn test_error_batch_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);
    client.pause();

    let mut items: Vec<BatchMintItem> = Vec::new(&env);
    items.push_back(BatchMintItem {
        asset: sample_asset(&env),
        metadata: String::from_str(&env, "meta"),
    });
    let result = client.try_batch_mint(&Address::generate(&env), &items);
    assert_err(result, Error::ContractPaused);
}

// ---------------------------------------------------------------------------
// Frozen token errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_frozen_already_frozen() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    client.freeze(&token_id);

    let result = client.try_freeze(&token_id);
    assert_err(result, Error::TokenFrozen);
}

#[test]
fn test_error_frozen_not_frozen() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));

    let result = client.try_unfreeze(&token_id);
    assert_err(result, Error::InvalidParameter);
}

// ---------------------------------------------------------------------------
// Unauthorized access errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_unauthorized_admin_action() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let attacker = Address::generate(&env);
    let result = client.try_pause_as(&attacker);
    assert_err(result, Error::Unauthorized);
}

#[test]
fn test_error_unauthorized_burn() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let attacker = Address::generate(&env);

    let result = client.try_burn(&attacker, &token_id);
    assert_err(result, Error::Unauthorized);
}

// ---------------------------------------------------------------------------
// Unsupported asset errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_unsupported_asset_carried() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let bad = Asset::Token(Address::generate(&env));
    let result = client.try_mint(&Address::generate(&env), &bad);
    match result {
        Ok(_) => panic!("expected UnsupportedAsset error"),
        Err(err) => {
            assert_eq!(err, Error::UnsupportedAsset);
            assert_eq!(client.get_last_unsupported_asset(), bad);
        }
    }
}

// ---------------------------------------------------------------------------
// Invalid parameter errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_invalid_parameter_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_withdraw(&Address::generate(&env), &0);
    assert_err(result, Error::InvalidParameter);
}

#[test]
fn test_error_invalid_parameter_bad_token_id() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_burn(&Address::generate(&env), &0);
    assert_err(result, Error::InvalidParameter);
}

// ---------------------------------------------------------------------------
// Contract pause state errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_pause_already_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);
    client.pause();

    let result = client.try_pause();
    assert_err(result, Error::ContractPaused);
}

#[test]
fn test_error_pause_not_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_unpause();
    assert_err(result, Error::InvalidParameter);
}

// ---------------------------------------------------------------------------
// Duplicate operation errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_duplicate_listing() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    client.list(&owner, &token_id, &100);

    let result = client.try_list(&owner, &token_id, &200);
    assert_err(result, Error::AlreadyListed);
}

#[test]
fn test_error_duplicate_approval() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let owner = Address::generate(&env);
    let token_id = client.mint(&owner, &sample_asset(&env));
    let spender = Address::generate(&env);
    client.approve(&owner, &spender, &token_id);

    let result = client.try_approve(&owner, &spender, &token_id);
    assert_err(result, Error::AlreadyApproved);
}

// ---------------------------------------------------------------------------
// Storage failure errors
// ---------------------------------------------------------------------------

#[test]
fn test_error_storage_missing_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    let result = client.try_get_listing(&9999);
    assert_err(result, Error::StorageError);
}

#[test]
fn test_error_storage_corrupt_state() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin) = setup_contract(&env);

    test_helpers::corrupt_storage(&env);
    let result = client.try_get_config();
    assert_err(result, Error::StorageError);
}

// ---------------------------------------------------------------------------
// Error registry coverage
// ---------------------------------------------------------------------------

#[test]
fn test_error_registry_covers_all_variants() {
    let env = Env::default();
    let (client, _admin) = setup_contract(&env);

    let registry = client.get_error_registry();
    let all_variants = Error::all_variants();

    assert_eq!(
        registry.len(),
        all_variants.len(),
        "registry size does not match number of error variants"
    );

    for variant in all_variants.iter() {
        assert!(
            registry.contains(variant),
            "error registry missing variant {:?}",
            variant
        );
    }
}

#[test]
fn test_error_codes_are_unique() {
    let all = Error::all_variants();
    let mut seen: Vec<u32> = Vec::new(&Env::default());
    for variant in all.iter() {
        let code = variant as u32;
        assert!(
            !seen.contains(&code),
            "duplicate error code {} for variant {:?}",
            code,
            variant
        );
        seen.push_back(code);
    }
}