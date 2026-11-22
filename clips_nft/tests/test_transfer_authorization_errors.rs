//! Tests for transfer authorization error handling.
//!
//! Validates that the contract returns correct standardized errors when:
//! - An account without transfer authorization attempts to move an NFT (`UnauthorizedTransfer`)
//! - An operator attempts NFT transfers without valid approval (`OperatorNotApproved`)
//! - Attempts to revoke or modify an approval that does not exist (`ApprovalNotFound`)
//!
//! These tests verify the acceptance criteria defined in the related issues.

use clips_nft::{AtomicMintContract, AtomicMintContractClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

/// Helper to set up a contract with a minted NFT.
fn setup_with_minted_nft(env: &Env) -> (Address, Address, Address, u32) {
    let admin = Address::generate(env);
    let creator = Address::generate(env);
    let owner = Address::generate(env);
    let contract_id = env.register(AtomicMintContract, ());
    let client = AtomicMintContractClient::new(env, &contract_id);

    client.initialize(&admin, &1000, &100, &10);

    // Mint an NFT to the owner
    env.mock_all_auths();
    let clip_id = 42u32;
    let metadata_uri = String::from_str(env, "ipfs://metadata");
    let token_id = client.mint(&owner, &clip_id, &metadata_uri);

    (owner, creator, contract_id, token_id)
}

// ────────────────────────────────────────────────────────────────────────────
// UnauthorizedTransfer: caller without transfer authorization
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_unauthorized_transfer_non_owner_non_operator() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let unauthorized_caller = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.mock_all_auths();

    // Attempt transfer by unauthorized caller (not owner, not operator)
    let result = client.try_transfer(&unauthorized_caller, &recipient, &token_id);

    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::UnauthorizedTransfer)),
        "Expected UnauthorizedTransfer error for non-owner, non-operator"
    );
}

#[test]
fn test_unauthorized_transfer_caller_is_neither_owner_nor_approved_operator() {
    let env = Env::default();
    env.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let random_address = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Verify caller
    env.mock_all_auths();
    let result = client.try_transfer(&random_address, &recipient, &token_id);

    // Return standardized error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::UnauthorizedTransfer)),
        "Transfer by unauthorized account must fail with UnauthorizedTransfer"
    );
}

#[test]
fn test_owner_can_transfer_without_unauthorized_error() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let recipient = Address::generate(&env);

    env.mock_all_auths();

    // Owner should be able to transfer
    client.transfer(&owner, &recipient, &token_id);

    // Verify transfer succeeded by checking new owner
    let new_owner = client.owner_of(&token_id);
    assert_eq!(new_owner, recipient);
}

#[test]
fn test_approved_operator_can_transfer_without_unauthorized_error() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let operator = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.mock_all_auths();

    // Owner approves operator
    client.approve_for_all(&owner, &operator, &true);

    // Operator should be able to transfer
    client.transfer_as_operator(&operator, &owner, &recipient, &token_id);

    // Verify transfer succeeded
    let new_owner = client.owner_of(&token_id);
    assert_eq!(new_owner, recipient);
}

// ────────────────────────────────────────────────────────────────────────────
// OperatorNotApproved: operator attempting transfer without valid approval
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_operator_not_approved_transfer_fails() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let unapproved_operator = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.mock_all_auths();

    // Detect missing approval
    let result = client.try_transfer_as_operator(&unapproved_operator, &owner, &recipient, &token_id);

    // Return error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::OperatorNotApproved)),
        "Unapproved operator must fail with OperatorNotApproved"
    );
}

#[test]
fn test_operator_not_approved_after_revocation() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let operator = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.mock_all_auths();

    // First approve operator
    client.approve_for_all(&owner, &operator, &true);

    // Then revoke approval
    client.revoke_operator_approval(&owner, &operator);

    // Detect missing approval after revocation
    let result = client.try_transfer_as_operator(&operator, &owner, &recipient, &token_id);

    // Return error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::OperatorNotApproved)),
        "Revoked operator must fail with OperatorNotApproved"
    );
}

#[test]
fn test_operator_cannot_equal_owner() {
    let env = Env::default();
    let (owner, _, contract_id, _token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    env.mock_all_auths();

    // Attempt to set owner as their own operator (self-approval)
    let result = client.try_approve_for_all(&owner, &owner, &true);

    // This should fail with OperatorNotApproved (self-approval check)
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::OperatorNotApproved)),
        "Self-approval must fail with OperatorNotApproved"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// ApprovalNotFound: revoking/modifying non-existent approval
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_revoke_nonexistent_token_approval() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    env.mock_all_auths();

    // Detect missing approval
    let result = client.try_revoke_approval(&owner, &token_id);

    // Return standardized error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::ApprovalNotFound)),
        "Revoking non-existent token approval must fail with ApprovalNotFound"
    );
}

#[test]
fn test_revoke_nonexistent_operator_approval() {
    let env = Env::default();
    let (owner, _, contract_id, _token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let operator = Address::generate(&env);

    env.mock_all_auths();

    // Detect missing approval
    let result = client.try_revoke_operator_approval(&owner, &operator);

    // Return standardized error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::ApprovalNotFound)),
        "Revoking non-existent operator approval must fail with ApprovalNotFound"
    );
}

#[test]
fn test_revoke_already_revoked_approval() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let approved_account = Address::generate(&env);

    env.mock_all_auths();

    // First approve
    client.approve(&owner, &approved_account, &token_id);

    // First revocation should succeed
    client.revoke_approval(&owner, &token_id);

    // Detect missing approval on second revocation
    let result = client.try_revoke_approval(&owner, &token_id);

    // Return standardized error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::ApprovalNotFound)),
        "Double revocation must fail with ApprovalNotFound"
    );
}

#[test]
fn test_revoke_operator_after_already_revoked() {
    let env = Env::default();
    let (owner, _, contract_id, _token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let operator = Address::generate(&env);

    env.mock_all_auths();

    // First approve operator
    client.approve_for_all(&owner, &operator, &true);

    // First revocation should succeed
    client.revoke_operator_approval(&owner, &operator);

    // Detect missing approval on second revocation
    let result = client.try_revoke_operator_approval(&owner, &operator);

    // Return standardized error
    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::ApprovalNotFound)),
        "Double operator revocation must fail with ApprovalNotFound"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Combined scenarios: multiple error conditions
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_error_priority_unauthorized_over_approval_not_found() {
    // When both conditions could apply (unauthorized caller AND no approval),
    // UnauthorizedTransfer should be returned
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let unauthorized = Address::generate(&env);
    let recipient = Address::generate(&env);

    env.mock_all_auths();

    let result = client.try_transfer(&unauthorized, &recipient, &token_id);

    assert_eq!(
        result,
        Err(Ok(clips_nft::types::Error::UnauthorizedTransfer)),
        "UnauthorizedTransfer should be checked before approval status"
    );
}

#[test]
fn test_successful_approval_flow_no_errors() {
    let env = Env::default();
    let (owner, _, contract_id, token_id) = setup_with_minted_nft(&env);
    let client = AtomicMintContractClient::new(&env, &contract_id);

    let approved = Address::generate(&env);

    env.mock_all_auths();

    // Approve should succeed without errors
    client.approve(&owner, &approved, &token_id);

    // Verify approval exists
    let approval = client.get_approved(&token_id);
    assert_eq!(approval, approved);

    // Revoke should succeed without errors
    client.revoke_approval(&owner, &token_id);

    // Attempting to get approval on revoked token should fail appropriately
    let result = client.try_get_approved(&token_id);
    assert!(result.is_err());
}
