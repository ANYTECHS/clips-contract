#![cfg(test)]

use clips_nft::{Error, types::Config};
use soroban_sdk::{testutils::Address as _, Address, String};

mod test_helpers;
use test_helpers::{setup, mint_clip};

// ─── Acceptance Criteria 1 & 4: Unauthorized Callers & Administrative Ops ───

#[test]
fn test_unauthorized_administrative_operations() {
    let ctx = setup();
    let unauthorized_caller = Address::generate(ctx.env);

    // Test set_config
    let config = Config {
        admin: unauthorized_caller.clone(),
        max_royalty_bps: 1000,
        mint_cooldown_secs: 0,
        platform_fee_bps: 0,
    };
    assert_eq!(
        ctx.client.try_set_config(&unauthorized_caller, &config),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );

    // Test pause
    assert_eq!(
        ctx.client.try_pause(&unauthorized_caller, &None),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );

    // Test set default royalty
    assert_eq!(
        ctx.client.try_set_default_royalty_bps(&unauthorized_caller, &500),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );

    // Test add currency
    let currency = Address::generate(ctx.env);
    assert_eq!(
        ctx.client.try_add_currency(&unauthorized_caller, &currency),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
}

#[test]
fn test_unauthorized_callers_rejected_for_protected_ops() {
    let ctx = setup();
    let owner = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &owner, 1, false);
    let malicious = Address::generate(ctx.env);
    let to = Address::generate(ctx.env);

    // Try to transfer someone else's token without approval
    // Note: The caller parameter in the contract is malicious, but the owner parameter is also provided.
    // In soroban, when mock_all_auths() is true, require_auth doesn't panic, but we have guards checking the explicit caller.
    // transfer_as_operator is a specific endpoint guarded by require_operator_authorized.
    assert_eq!(
        ctx.client.try_transfer_as_operator(&malicious, &owner, &to, &token_id),
        Err(Ok(Error::OperatorNotApproved))
    );
}

// ─── Acceptance Criteria 2: Invalid Token States ───────────────────────────

#[test]
fn test_invalid_token_states() {
    let ctx = setup();
    let user = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &user, 1, false);

    // Admin freezes the token
    let reason = String::from_str(ctx.env, "security violation");
    ctx.client.freeze_token(&ctx.admin, &token_id, &Some(reason));

    let to = Address::generate(ctx.env);

    // Try transfer on frozen token
    let res = ctx.client.try_transfer(&user, &user, &to, &token_id);
    assert_eq!(res, Err(Ok(Error::InvalidTransferState)));

    // Try freeze again
    let reason_again = String::from_str(ctx.env, "second freeze");
    let freeze_res = ctx.client.try_freeze_token(&ctx.admin, &token_id, &Some(reason_again));
    assert_eq!(freeze_res, Err(Ok(Error::AlreadyFrozen)));
}

// ─── Acceptance Criteria 3: Paused Contract Operations ─────────────────────

#[test]
fn test_paused_contract_operations() {
    let ctx = setup();
    let user = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &user, 1, false);
    
    // Pause contract as admin
    ctx.client.pause(&ctx.admin, &None);

    let to = Address::generate(ctx.env);
    let operator = Address::generate(ctx.env);
    
    // Test transfer fails when paused
    assert_eq!(
        ctx.client.try_transfer(&user, &user, &to, &token_id),
        Err(Ok(Error::ContractPaused))
    );

    // Test approve fails when paused
    assert_eq!(
        ctx.client.try_approve(&user, &operator, &token_id),
        Err(Ok(Error::ContractPaused))
    );

    // Test set_approval_for_all fails when paused
    assert_eq!(
        ctx.client.try_set_approval_for_all(&user, &operator, &true),
        Err(Ok(Error::ContractPaused))
    );

    // Unpause and verify operations succeed
    ctx.client.unpause(&ctx.admin);
    
    // Transfer should now succeed
    ctx.client.transfer(&user, &user, &to, &token_id);
}

// ─── Acceptance Criteria 5: Verify Guards Execution ────────────────────────

#[test]
fn test_all_protected_functions_execute_guards() {
    let ctx = setup();
    let user = Address::generate(ctx.env);
    let malicious = Address::generate(ctx.env);
    let token_id = mint_clip(&ctx, &user, 1, false);
    
    // Pause to test guards globally across multiple functions
    ctx.client.pause(&ctx.admin, &None);

    // Both authorization and pause guards should be active. 
    // We check that standard protected functions return ContractPaused (or similar guard-triggered errors).

    // 1. revoke_approval
    assert_eq!(
        ctx.client.try_revoke_approval(&user, &token_id),
        Err(Ok(Error::ContractPaused))
    );

    // 2. revoke_operator_approval
    let operator = Address::generate(ctx.env);
    assert_eq!(
        ctx.client.try_revoke_operator_approval(&user, &operator),
        Err(Ok(Error::ContractPaused))
    );

    // 3. transfer_from
    let to = Address::generate(ctx.env);
    assert_eq!(
        ctx.client.try_transfer_from(&user, &user, &to, &token_id),
        Err(Ok(Error::ContractPaused))
    );

    // Unpause to test specific state guards (like frozen)
    ctx.client.unpause(&ctx.admin);

    let reason = String::from_str(ctx.env, "frozen for test");
    ctx.client.freeze_token(&ctx.admin, &token_id, &Some(reason));

    // transfer_as_operator on frozen token
    assert_eq!(
        ctx.client.try_transfer_as_operator(&ctx.admin, &user, &to, &token_id),
        Err(Ok(Error::InvalidTransferState))
    );
}
