//! Comprehensive guard test suite — covers all guards and their interactions.
//! Issues #1034, #1035, #1036, #1037.

use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

use crate::{
    blacklist, config_guard, frozen_token, operator_approval, pause_guard, pause_state,
    royalty_admin_guard, royalty_pause_guard, storage_guard, token_approval, token_owner_storage,
    transfer_guard,
    types::{DataKey, Error, Royalty, RoyaltyRecipient, TokenData, TokenId},
    ClipsNftContract,
};

// Helper to run inside contract context with mock auth
fn with_clips<F, R>(f: F) -> R
where
    F: FnOnce(&Env, &Address) -> R,
{
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    env.as_contract(&cid, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
        f(&env, &admin)
    })
}

fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
    token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    env.storage().persistent().set(
        &DataKey::Token(token_id),
        &TokenData {
            owner: owner.clone(),
            clip_id: token_id,
        },
    );
    let _ = crate::wallet_token_index::add_token_to_wallet(env, owner, token_id);
}

// ── Pause guard independently ─────────────────────────────────────────────
#[test]
fn pause_guard_blocks_when_paused() {
    with_clips(|env, _| {
        pause_state::save_pause_state(env, true);
        assert_eq!(
            pause_guard::require_not_paused(env),
            Err(Error::ContractPaused)
        );
    });
}

#[test]
fn pause_guard_passes_when_not_paused() {
    with_clips(|env, _| {
        pause_state::save_pause_state(env, false);
        assert!(pause_guard::require_not_paused(env).is_ok());
    });
}

#[test]
fn royalty_pause_guard_blocks_when_paused() {
    with_clips(|env, _| {
        pause_state::save_pause_state(env, true);
        assert_eq!(
            royalty_pause_guard::require_royalty_not_paused(env),
            Err(Error::ContractPaused)
        );
    });
}

// ── Frozen guard ──────────────────────────────────────────────────────────
#[test]
fn frozen_blocks_transfer() {
    with_clips(|env, _| {
        let owner = Address::generate(env);
        let to = Address::generate(env);
        setup_token(env, 1, &owner);
        frozen_token::freeze_token(env, 1);
        assert_eq!(
            transfer_guard::check_not_frozen(env, 1),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            transfer_guard::check_transfer(env, &owner, &owner, &to, 1),
            Err(Error::Unauthorized)
        );
    });
}

// ── Blacklist ─────────────────────────────────────────────────────────────
#[test]
fn blacklist_blocks_sender_and_recipient() {
    with_clips(|env, _| {
        let from = Address::generate(env);
        let to = Address::generate(env);
        setup_token(env, 2, &from);
        blacklist::add_wallet(env, &from);
        assert_eq!(
            transfer_guard::check_not_blacklisted(env, &from, &to),
            Err(Error::InvalidAddress)
        );
        blacklist::remove_wallet(env, &from);
        blacklist::add_wallet(env, &to);
        assert_eq!(
            transfer_guard::check_not_blacklisted(env, &from, &to),
            Err(Error::InvalidAddress)
        );
    });
}

// ── Self-transfer and valid recipient ─────────────────────────────────────
#[test]
fn self_transfer_guard_rejects() {
    with_clips(|env, _| {
        let a = Address::generate(env);
        assert_eq!(
            transfer_guard::check_not_self_transfer(&a, &a),
            Err(Error::SelfTransferNotAllowed)
        );
    });
}

#[test]
fn contract_as_recipient_rejected() {
    with_clips(|env, _| {
        let contract = env.current_contract_address();
        assert_eq!(
            transfer_guard::check_valid_recipient(env, &contract),
            Err(Error::InvalidRecipient)
        );
    });
}

// ── Caller authorized ─────────────────────────────────────────────────────
#[test]
fn caller_authorized_owner_passes() {
    with_clips(|env, _| {
        let owner = Address::generate(env);
        setup_token(env, 10, &owner);
        assert!(transfer_guard::check_caller_authorized(env, &owner, &owner, 10).is_ok());
    });
}

#[test]
fn caller_authorized_operator_passes() {
    with_clips(|env, _| {
        let owner = Address::generate(env);
        let op = Address::generate(env);
        setup_token(env, 11, &owner);
        operator_approval::save_operator(env, &owner, &op);
        assert!(transfer_guard::check_caller_authorized(env, &op, &owner, 11).is_ok());
    });
}

#[test]
fn caller_authorized_fails_for_stranger() {
    with_clips(|env, _| {
        let owner = Address::generate(env);
        let stranger = Address::generate(env);
        setup_token(env, 12, &owner);
        assert_eq!(
            transfer_guard::check_caller_authorized(env, &stranger, &owner, 12),
            Err(Error::Unauthorized)
        );
    });
}

// ── Config guard ──────────────────────────────────────────────────────────
#[test]
fn config_guard_only_admin() {
    with_clips(|env, admin| {
        let other = Address::generate(env);
        assert!(config_guard::require_config_admin(env, admin).is_ok());
        assert_eq!(
            config_guard::require_config_admin(env, &other),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

// ── Royalty admin guard ───────────────────────────────────────────────────
#[test]
fn royalty_admin_guard_only_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    env.as_contract(&cid, || {
        crate::administrator_storage::add_admin(&env, &admin);
        assert!(royalty_admin_guard::require_royalty_admin(&env, &admin).is_ok());
        assert_eq!(
            royalty_admin_guard::require_royalty_admin(&env, &user),
            Err(Error::Unauthorized)
        );
    });
}

// ── Storage guard ─────────────────────────────────────────────────────────
#[test]
fn storage_guard_token_owner() {
    with_clips(|env, admin| {
        let owner = Address::generate(env);
        setup_token(env, 20, &owner);
        assert!(storage_guard::guard_token_owner(env, &owner, 20).is_ok());
        let other = Address::generate(env);
        assert_eq!(
            storage_guard::guard_token_owner(env, &other, 20),
            Err(Error::Unauthorized)
        );
        assert_eq!(storage_guard::guard_admin(env, admin), Ok(()));
    });
}

#[test]
fn storage_guard_not_paused() {
    with_clips(|env, _| {
        env.storage().instance().set(&DataKey::Paused, &true);
        assert_eq!(
            storage_guard::guard_not_paused(env),
            Err(Error::ContractPaused)
        );
        env.storage().instance().set(&DataKey::Paused, &false);
        assert!(storage_guard::guard_not_paused(env).is_ok());
    });
}

// ── Multiple guards together ──────────────────────────────────────────────
#[test]
fn multiple_guards_paused_and_frozen() {
    with_clips(|env, _| {
        let owner = Address::generate(env);
        let to = Address::generate(env);
        setup_token(env, 30, &owner);
        pause_state::save_pause_state(env, true);
        frozen_token::freeze_token(env, 30);
        // Both guards should block; pause first
        assert_eq!(
            pause_guard::require_not_paused(env),
            Err(Error::ContractPaused)
        );
        assert_eq!(
            transfer_guard::check_transfer(env, &owner, &owner, &to, 30),
            Err(Error::Unauthorized)
        );
        // Unpause and unfreeze should allow
        pause_state::save_pause_state(env, false);
        frozen_token::unfreeze_token(env, 30);
        assert!(transfer_guard::check_transfer(env, &owner, &owner, &to, 30).is_ok());
    });
}

#[test]
fn paused_blocks_marketplace_and_royalty() {
    with_clips(|env, admin| {
        let owner = Address::generate(env);
        setup_token(env, 40, &owner);
        pause_state::save_pause_state(env, true);
        // marketplace list should be blocked via pause_guard inside validate_listing
        let asset = Address::generate(env);
        crate::payment_currency::add_currency(env, asset.clone()).unwrap();
        let req = crate::listing_request::ListingRequest {
            listing_id: 0,
            token_id: 40,
            price: 1000,
            payment_asset: asset,
            expiration: 0,
            seller: owner.clone(),
        };
        assert_eq!(
            crate::marketplace::listing_validator::validate_listing(
                env,
                &owner,
                40,
                1000,
                &req.payment_asset,
                0
            ),
            Err(Error::ContractPaused)
        );
        // royalty should be blocked
        assert_eq!(
            royalty_pause_guard::require_royalty_not_paused(env),
            Err(Error::ContractPaused)
        );
        // unpause allows
        pause_state::save_pause_state(env, false);
        assert!(royalty_pause_guard::require_royalty_not_paused(env).is_ok());
        let _ = admin;
    });
}

// ── Transfer integration via contract entry points ────────────────────────
#[test]
fn transfer_integration_success_and_failure() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    let client = crate::ClipsNftContractClient::new(&env, &cid);
    env.as_contract(&cid, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
    });
    // Need to mint token via direct storage for transfer tests
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let stranger = Address::generate(&env);
    env.as_contract(&cid, || {
        setup_token(&env, 100, &owner);
    });
    // Successful transfer by owner
    let res = client.try_transfer(&owner, &owner, &to, &100);
    assert!(res.is_ok());
    // Verify new owner
    env.as_contract(&cid, || {
        assert_eq!(token_owner_storage::get_owner(&env, 100).unwrap(), to);
    });
    // Setup again for failure case: frozen token
    env.as_contract(&cid, || {
        setup_token(&env, 101, &owner);
        frozen_token::freeze_token(&env, 101);
    });
    let res2 = client.try_transfer(&owner, &owner, &to, &101);
    assert_eq!(res2, Err(Ok(Error::Unauthorized)));
    // Blacklisted sender
    env.as_contract(&cid, || {
        setup_token(&env, 102, &owner);
        blacklist::add_wallet(&env, &owner);
    });
    let res3 = client.try_transfer(&owner, &owner, &to, &102);
    assert_eq!(res3, Err(Ok(Error::InvalidAddress)));
    // Clean up blacklist for next
    env.as_contract(&cid, || {
        blacklist::remove_wallet(&env, &owner);
    });
    // Invalid caller
    env.as_contract(&cid, || {
        setup_token(&env, 103, &owner);
    });
    let res4 = client.try_transfer(&stranger, &owner, &to, &103);
    assert_eq!(res4, Err(Ok(Error::Unauthorized)));
}

// ── Batch transfer guard ──────────────────────────────────────────────────
#[test]
fn batch_transfer_guard_multiple() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    let client = crate::ClipsNftContractClient::new(&env, &cid);
    env.as_contract(&cid, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
    });
    let owner = Address::generate(&env);
    let to1 = Address::generate(&env);
    let to2 = Address::generate(&env);
    env.as_contract(&cid, || {
        setup_token(&env, 200, &owner);
        setup_token(&env, 201, &owner);
    });
    let mut reqs = Vec::new(&env);
    reqs.push_back(crate::transfer_request::TransferRequest {
        token_id: 200,
        from: owner.clone(),
        to: to1.clone(),
        timestamp: 0,
        memo: None,
    });
    reqs.push_back(crate::transfer_request::TransferRequest {
        token_id: 201,
        from: owner.clone(),
        to: to2.clone(),
        timestamp: 0,
        memo: None,
    });
    let batch = crate::transfer_request::BatchTransferRequest { requests: reqs };
    let _res = client.batch_transfer(&owner, &batch);
    // Batch with one frozen should fail
    env.as_contract(&cid, || {
        setup_token(&env, 202, &owner);
        setup_token(&env, 203, &owner);
        frozen_token::freeze_token(&env, 203);
    });
    let mut reqs2 = Vec::new(&env);
    reqs2.push_back(crate::transfer_request::TransferRequest {
        token_id: 202,
        from: owner.clone(),
        to: to1.clone(),
        timestamp: 0,
        memo: None,
    });
    reqs2.push_back(crate::transfer_request::TransferRequest {
        token_id: 203,
        from: owner.clone(),
        to: to2.clone(),
        timestamp: 0,
        memo: None,
    });
    let batch2 = crate::transfer_request::BatchTransferRequest { requests: reqs2 };
    let res2 = client.try_batch_transfer(&owner, &batch2);
    assert_eq!(res2, Err(Ok(Error::Unauthorized)));
}

// ── Marketplace guard integration ─────────────────────────────────────────
#[test]
fn marketplace_guards_protect_listing_and_purchase() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    let client = crate::ClipsNftContractClient::new(&env, &cid);
    env.as_contract(&cid, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTokenId, &10u32);
    });
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let asset = Address::generate(&env);
    env.as_contract(&cid, || {
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        setup_token(&env, 10, &seller);
    });
    // Successful listing when not paused/frozen
    let req = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: 10,
        price: 1000,
        payment_asset: asset.clone(),
        expiration: 0,
        seller: seller.clone(),
    };
    assert!(client.try_list_nft(&req).is_ok());
    // Paused blocks listing
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    let req2 = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: 11,
        price: 1000,
        payment_asset: asset.clone(),
        expiration: 0,
        seller: seller.clone(),
    };
    env.as_contract(&cid, || {
        setup_token(&env, 11, &seller);
    });
    assert_eq!(client.try_list_nft(&req2), Err(Ok(Error::ContractPaused)));
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    // Frozen blocks listing
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 11));
    assert_eq!(client.try_list_nft(&req2), Err(Ok(Error::Unauthorized)));
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 11));
    assert!(client.try_list_nft(&req2).is_ok());
    // Blacklisted seller blocked
    env.as_contract(&cid, || blacklist::add_wallet(&env, &seller));
    let req3 = crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id: 12,
        price: 1000,
        payment_asset: asset.clone(),
        expiration: 0,
        seller: seller.clone(),
    };
    env.as_contract(&cid, || setup_token(&env, 12, &seller));
    assert_eq!(client.try_list_nft(&req3), Err(Ok(Error::InvalidAddress)));
    env.as_contract(&cid, || blacklist::remove_wallet(&env, &seller));
}

// ── Royalty guard integration ─────────────────────────────────────────────
#[test]
fn royalty_guards_protect_config_and_payment() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    let client = crate::ClipsNftContractClient::new(&env, &cid);
    env.as_contract(&cid, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
        setup_token(&env, 50, &admin);
    });
    let royalty = crate::types::Royalty {
        recipients: Vec::new(&env),
        asset_address: None,
    };
    // Need valid royalty: at least one recipient for set_royalty success, but we test guards
    // Paused blocks royalty config
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_set_royalty(&admin, &50, &royalty),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    // Frozen token blocks royalty config
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 50));
    assert_eq!(
        client.try_set_royalty(&admin, &50, &royalty),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 50));
    // Non-admin blocked
    let other = Address::generate(&env);
    assert_eq!(
        client.try_set_royalty(&other, &50, &royalty),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
    // Valid admin + not paused/frozen should pass validation (even if royalty invalid, guard passes first)
    // Use valid royalty
    let recipient = Address::generate(&env);
    let mut recs = Vec::new(&env);
    recs.push_back(crate::types::RoyaltyRecipient {
        recipient: recipient.clone(),
        basis_points: 100,
    });
    let valid_royalty = crate::types::Royalty {
        recipients: recs,
        asset_address: None,
    };
    assert!(client.try_set_royalty(&admin, &50, &valid_royalty).is_ok());
    // Pay royalty paused blocks
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    let payer = Address::generate(&env);
    assert_eq!(
        client.try_pay_royalty(&payer, &50, &1000),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
}

// ── Edge cases and invalid callers ────────────────────────────────────────
#[test]
fn edge_cases_invalid_callers_and_states() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    let client = crate::ClipsNftContractClient::new(&env, &cid);
    env.as_contract(&cid, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
    });
    // Token not found
    let fake = Address::generate(&env);
    let to = Address::generate(&env);
    assert_eq!(
        client.try_transfer(&fake, &fake, &to, &9999),
        Err(Ok(Error::TokenNotFound))
    );
    // Self-transfer
    let owner = Address::generate(&env);
    env.as_contract(&cid, || setup_token(&env, 60, &owner));
    assert_eq!(
        client.try_transfer(&owner, &owner, &owner, &60),
        Err(Ok(Error::SelfTransferNotAllowed))
    );
    // Contract as recipient
    let contract_addr = cid.clone();
    assert_eq!(
        client.try_transfer(&owner, &owner, &contract_addr, &60),
        Err(Ok(Error::InvalidRecipient))
    );
    // Batch empty
    let batch = crate::transfer_request::BatchTransferRequest {
        requests: Vec::new(&env),
    };
    assert_eq!(
        client.try_batch_transfer(&owner, &batch),
        Err(Ok(Error::InvalidConfig))
    );
}

// ── Mint authorization guard (issue #1081) ─────────────────────────────────
#[test]
fn mint_auth_guard_admin_and_minter_paths() {
    with_clips(|env, admin| {
        // Admin passes.
        assert!(crate::mint_authorization::require_mint_auth(env, admin).is_ok());
        // Stranger fails.
        let stranger = Address::generate(env);
        assert_eq!(
            crate::mint_authorization::require_mint_auth(env, &stranger),
            Err(Error::UnauthorizedMinter)
        );
        // Approved minter passes; revoked minter fails.
        crate::mint_authorization::set_approved_minter(env, &stranger);
        assert!(crate::mint_authorization::require_mint_auth(env, &stranger).is_ok());
        crate::mint_authorization::remove_approved_minter(env, &stranger);
        assert_eq!(
            crate::mint_authorization::require_mint_auth(env, &stranger),
            Err(Error::UnauthorizedMinter)
        );
    });
}

#[test]
fn mint_auth_guard_rejects_when_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let caller = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    env.as_contract(&cid, || {
        // No DataKey::Admin set — guard must report NotInitialized.
        assert_eq!(
            crate::mint_authorization::require_mint_auth(&env, &caller),
            Err(Error::NotInitialized)
        );
    });
}

// ── Royalty freeze + emergency guards (issue #1081) ─────────────────────────
#[test]
fn royalty_freeze_guard_blocks_frozen_configs() {
    with_clips(|env, _| {
        let token_id = 70;
        assert!(crate::royalty_freeze::require_not_frozen(env, token_id).is_ok());
        env.storage().persistent().set(
            &DataKey::RoyaltyFrozen(token_id),
            &true,
        );
        assert_eq!(
            crate::royalty_freeze::require_not_frozen(env, token_id),
            Err(Error::RoyaltyFrozen)
        );
    });
}

#[test]
fn royalty_emergency_guard_blocks_when_disabled() {
    with_clips(|env, admin| {
        assert!(crate::royalty_emergency::require_payments_enabled(env).is_ok());
        crate::royalty_emergency::set_payments_disabled(env, admin, true).unwrap();
        assert_eq!(
            crate::royalty_emergency::require_payments_enabled(env),
            Err(Error::RoyaltyPaymentsDisabled)
        );
        crate::royalty_emergency::set_payments_disabled(env, admin, false).unwrap();
        assert!(crate::royalty_emergency::require_payments_enabled(env).is_ok());
    });
}

#[test]
fn royalty_emergency_toggle_requires_admin() {
    with_clips(|env, _| {
        let intruder = Address::generate(env);
        assert_eq!(
            crate::royalty_emergency::set_payments_disabled(env, &intruder, true),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}

// ── Royalty validation pipeline stages (issue #1081) ────────────────────────
#[test]
fn royalty_pipeline_rejects_paused_frozen_unauthorized() {
    with_clips(|env, admin| {
        let owner = Address::generate(env);
        let token_id = 71;
        setup_token(env, token_id, &owner);
        // Seed a royalty config so token-exists stages pass.
        let mut recs = Vec::new(env);
        recs.push_back(RoyaltyRecipient {
            recipient: owner.clone(),
            basis_points: 100,
        });
        let royalty = Royalty {
            recipients: recs,
            asset_address: None,
        };
        crate::token_storage::set_royalty(env, token_id, &royalty);

        // Success path for the owner.
        assert!(crate::royalty_validation_pipeline::validate_royalty_operation(
            env, &owner, token_id, &royalty
        )
        .is_ok());

        // Paused contracts reject everything first.
        pause_state::save_pause_state(env, true);
        assert_eq!(
            crate::royalty_validation_pipeline::validate_royalty_operation(
                env, &owner, token_id, &royalty
            ),
            Err(Error::ContractPaused)
        );
        pause_state::save_pause_state(env, false);

        // Frozen royalty configs are rejected.
        env.storage().persistent().set(
            &DataKey::RoyaltyFrozen(token_id),
            &true,
        );
        assert_eq!(
            crate::royalty_validation_pipeline::validate_royalty_operation(
                env, &owner, token_id, &royalty
            ),
            Err(Error::RoyaltyFrozen)
        );
        env.storage()
            .persistent()
            .remove(&DataKey::RoyaltyFrozen(token_id));

        // Invalid callers are rejected.
        let stranger = Address::generate(env);
        let _ = admin;
        assert_eq!(
            crate::royalty_validation_pipeline::validate_royalty_operation(
                env, &stranger, token_id, &royalty
            ),
            Err(Error::UnauthorizedConfigurationUpdate)
        );

        // Unknown tokens are rejected.
        assert_eq!(
            crate::royalty_validation_pipeline::validate_royalty_operation(
                env, &owner, 9999, &royalty
            ),
            Err(Error::TokenNotFound)
        );
    });
}

// ── Ownership + admin access guards (issue #1081) ───────────────────────────
#[test]
fn ownership_guard_accepts_owner_rejects_stranger_and_missing() {
    with_clips(|env, _| {
        let owner = Address::generate(env);
        let stranger = Address::generate(env);
        setup_token(env, 80, &owner);
        assert!(crate::ownership_guard::require_owner(env, &owner, 80).is_ok());
        assert_eq!(
            crate::ownership_guard::require_owner(env, &stranger, 80),
            Err(Error::Unauthorized)
        );
        assert_eq!(
            crate::ownership_guard::require_owner(env, &stranger, 9999),
            Err(Error::TokenNotFound)
        );
        assert!(crate::ownership_guard::check_caller_is_owner(env, &owner, 80));
        assert!(!crate::ownership_guard::check_caller_is_owner(
            env, &stranger, 80
        ));
    });
}

#[test]
fn admin_access_guard_accepts_admin_rejects_stranger() {
    with_clips(|env, admin| {
        assert!(crate::admin_access_control_guard::require_admin(env, admin).is_ok());
        let stranger = Address::generate(env);
        assert_eq!(
            crate::admin_access_control_guard::require_admin(env, &stranger),
            Err(Error::Unauthorized)
        );
        assert!(crate::admin_access_control_guard::check_caller_is_admin(env, admin));
        assert!(!crate::admin_access_control_guard::check_caller_is_admin(env, &stranger));
        assert_eq!(
            crate::admin_access_control_guard::get_configured_admin(env).unwrap(),
            *admin
        );
    });
}

// ── Guard composition: pause + admin + freeze (issue #1081) ─────────────────
#[test]
fn guard_composition_pause_admin_freeze_order() {
    with_clips(|env, admin| {
        let owner = Address::generate(env);
        setup_token(env, 90, &owner);
        // Happy path: all three guards pass in order.
        assert!(crate::guard_composition::GuardBuilder::new()
            .add(pause_guard::require_not_paused(env))
            .add(config_guard::require_config_admin(env, admin))
            .add(transfer_guard::check_not_frozen(env, 90))
            .execute()
            .is_ok());

        // Pause failure short-circuits before admin is evaluated.
        pause_state::save_pause_state(env, true);
        assert_eq!(
            crate::guard_composition::GuardBuilder::new()
                .add(pause_guard::require_not_paused(env))
                .add(config_guard::require_config_admin(env, admin))
                .execute(),
            Err(Error::ContractPaused)
        );
        pause_state::save_pause_state(env, false);

        // Frozen failure surfaces after pause + admin pass.
        frozen_token::freeze_token(env, 90);
        assert_eq!(
            crate::guard_composition::GuardBuilder::new()
                .add(pause_guard::require_not_paused(env))
                .add(config_guard::require_config_admin(env, admin))
                .add(transfer_guard::check_not_frozen(env, 90))
                .execute(),
            Err(Error::Unauthorized)
        );
        frozen_token::unfreeze_token(env, 90);

        // Invalid caller fails the admin stage.
        let stranger = Address::generate(env);
        assert_eq!(
            crate::guard_composition::GuardBuilder::new()
                .add(pause_guard::require_not_paused(env))
                .add(config_guard::require_config_admin(env, &stranger))
                .execute(),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    });
}
