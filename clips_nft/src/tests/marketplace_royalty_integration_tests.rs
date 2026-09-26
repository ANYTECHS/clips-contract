//! Marketplace + royalty guard integration tests.
//! Issues #1079 (marketplace guards) and #1080 (royalty guards).
//!
//! These tests exercise the contract entry points end-to-end through
//! `ClipsNftContractClient`, verifying that authorization and state guards
//! protect listing creation/cancellation, offer operations, purchase
//! settlement, royalty configuration/recipient updates, and royalty payments.
//!
//! Note: `create_listing` (backed by `crate::listing_storage`) shares its
//! storage with `cancel_listing` / `buy_listing`, while `list_nft` persists
//! through `marketplace::listing_storage`. Tests that later cancel or buy a
//! listing therefore use `create_listing` so setup and settlement read the
//! same record.

use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

use crate::{
    blacklist, frozen_token, operator_approval, pause_state, token_owner_storage,
    types::{DataKey, Error, Royalty, RoyaltyRecipient, TokenData, TokenId},
    ClipsNftContract, ClipsNftContractClient,
};

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

fn init_contract(env: &Env, cid: &Address, admin: &Address) {
    env.as_contract(cid, || {
        env.storage().instance().set(&DataKey::Admin, admin);
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &1000u32);
    });
}

fn add_supported_currency(env: &Env, cid: &Address) -> Address {
    let asset = Address::generate(env);
    env.as_contract(cid, || {
        crate::payment_currency::add_currency(env, asset.clone()).unwrap();
    });
    asset
}

fn royalty_with(env: &Env, recipient: &Address, bps: u32) -> Royalty {
    let mut recs = Vec::new(env);
    recs.push_back(RoyaltyRecipient {
        recipient: recipient.clone(),
        basis_points: bps,
    });
    Royalty {
        recipients: recs,
        asset_address: None,
    }
}

fn listing_req(
    seller: &Address,
    token_id: TokenId,
    price: i128,
    asset: &Address,
) -> crate::listing_request::ListingRequest {
    crate::listing_request::ListingRequest {
        listing_id: 0,
        token_id,
        price,
        payment_asset: asset.clone(),
        expiration: 0,
        seller: seller.clone(),
    }
}

// ── #1079: listing creation guards ────────────────────────────────────────────
#[test]
fn listing_creation_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    init_contract(&env, &cid, &admin);
    let client = ClipsNftContractClient::new(&env, &cid);

    let seller = Address::generate(&env);
    let asset = add_supported_currency(&env, &cid);
    env.as_contract(&cid, || setup_token(&env, 101, &seller));

    // Success path.
    assert!(client
        .try_list_nft(&listing_req(&seller, 101, 1000, &asset))
        .is_ok());

    // Duplicate active listing is rejected.
    assert_eq!(
        client.try_list_nft(&listing_req(&seller, 101, 1000, &asset)),
        Err(Ok(Error::DuplicateRecord))
    );

    // Non-owner cannot list.
    let stranger = Address::generate(&env);
    env.as_contract(&cid, || setup_token(&env, 102, &seller));
    assert_eq!(
        client.try_list_nft(&listing_req(&stranger, 102, 1000, &asset)),
        Err(Ok(Error::Unauthorized))
    );

    // Paused contract blocks listing creation.
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    env.as_contract(&cid, || setup_token(&env, 103, &seller));
    assert_eq!(
        client.try_list_nft(&listing_req(&seller, 103, 1000, &asset)),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));

    // Frozen tokens cannot be listed.
    env.as_contract(&cid, || {
        setup_token(&env, 104, &seller);
        frozen_token::freeze_token(&env, 104);
    });
    assert_eq!(
        client.try_list_nft(&listing_req(&seller, 104, 1000, &asset)),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || {
        frozen_token::unfreeze_token(&env, 104);
    });

    // Blacklisted sellers cannot list.
    env.as_contract(&cid, || {
        setup_token(&env, 105, &seller);
        blacklist::add_wallet(&env, &seller);
    });
    assert_eq!(
        client.try_list_nft(&listing_req(&seller, 105, 1000, &asset)),
        Err(Ok(Error::InvalidAddress))
    );
    env.as_contract(&cid, || {
        blacklist::remove_wallet(&env, &seller);
    });

    // Unsupported payment assets are rejected.
    let unsupported = Address::generate(&env);
    env.as_contract(&cid, || setup_token(&env, 106, &seller));
    assert_eq!(
        client.try_list_nft(&listing_req(&seller, 106, 1000, &unsupported)),
        Err(Ok(Error::UnsupportedAsset))
    );
}

// ── #1079: listing cancellation guards ────────────────────────────────────────
#[test]
fn listing_cancellation_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    init_contract(&env, &cid, &admin);
    let client = ClipsNftContractClient::new(&env, &cid);

    let seller = Address::generate(&env);
    let asset = add_supported_currency(&env, &cid);
    env.as_contract(&cid, || setup_token(&env, 201, &seller));
    client
        .try_create_listing(&listing_req(&seller, 201, 1000, &asset))
        .unwrap();

    // Stranger cannot cancel.
    let stranger = Address::generate(&env);
    assert_eq!(
        client.try_cancel_listing(&stranger, &201),
        Err(Ok(Error::Unauthorized))
    );

    // Operator approved by the seller can cancel.
    let operator = Address::generate(&env);
    env.as_contract(&cid, || operator_approval::save_operator(
        &env, &seller, &operator
    ));
    assert!(client.try_cancel_listing(&operator, &201).is_ok());

    // Re-list, then the contract admin can cancel.
    env.as_contract(&cid, || setup_token(&env, 202, &seller));
    client
        .try_create_listing(&listing_req(&seller, 202, 1000, &asset))
        .unwrap();
    assert!(client.try_cancel_listing(&admin, &202).is_ok());

    // Paused / frozen states block cancellation.
    env.as_contract(&cid, || setup_token(&env, 203, &seller));
    client
        .try_create_listing(&listing_req(&seller, 203, 1000, &asset))
        .unwrap();
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_cancel_listing(&seller, &203),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 203));
    assert_eq!(
        client.try_cancel_listing(&seller, &203),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 203));
    assert!(client.try_cancel_listing(&seller, &203).is_ok());
}

// ── #1079: offer operation guards ─────────────────────────────────────────────
#[test]
fn offer_operation_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    init_contract(&env, &cid, &admin);
    let client = ClipsNftContractClient::new(&env, &cid);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let asset = add_supported_currency(&env, &cid);
    env.as_contract(&cid, || {
        setup_token(&env, 301, &seller);
        crate::token_storage::set_royalty(&env, 301, &royalty_with(&env, &seller, 0));
    });

    // make_offer success path.
    assert!(client
        .try_make_offer(&buyer, &301, &500, &asset, &0u64)
        .is_ok());

    // Duplicate offer rejected.
    assert_eq!(
        client.try_make_offer(&buyer, &301, &500, &asset, &0u64),
        Err(Ok(Error::OfferAlreadyExists))
    );

    // cancel_offer: stranger rejected, operator accepted.
    let stranger = Address::generate(&env);
    assert_eq!(
        client.try_cancel_offer(&stranger, &301),
        Err(Ok(Error::Unauthorized))
    );
    let operator = Address::generate(&env);
    env.as_contract(&cid, || operator_approval::save_operator(
        &env, &buyer, &operator
    ));
    assert!(client.try_cancel_offer(&operator, &301).is_ok());

    // Re-offer, then buyer cancels directly.
    assert!(client
        .try_make_offer(&buyer, &301, &500, &asset, &0u64)
        .is_ok());
    assert!(client.try_cancel_offer(&buyer, &301).is_ok());

    // make_offer blocked when paused / frozen / blacklisted / bad price / bad asset.
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_make_offer(&buyer, &301, &500, &asset, &0u64),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 301));
    assert_eq!(
        client.try_make_offer(&buyer, &301, &500, &asset, &0u64),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 301));
    assert_eq!(
        client.try_make_offer(&buyer, &301, &0, &asset, &0u64),
        Err(Ok(Error::InvalidSalePrice))
    );
    let unsupported = Address::generate(&env);
    assert_eq!(
        client.try_make_offer(&buyer, &301, &500, &unsupported, &0u64),
        Err(Ok(Error::UnsupportedAsset))
    );

    // accept_offer: a non-owner cannot accept; self-dealing is rejected at
    // the validator level (buyer == seller).
    assert!(client
        .try_make_offer(&buyer, &301, &500, &asset, &0u64)
        .is_ok());
    assert_eq!(
        client.try_accept_offer(&buyer, &301),
        Err(Ok(Error::Unauthorized))
    );
}

// ── #1079: purchase settlement guards ─────────────────────────────────────────
#[test]
fn purchase_settlement_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    init_contract(&env, &cid, &admin);
    let client = ClipsNftContractClient::new(&env, &cid);

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let asset = add_supported_currency(&env, &cid);
    env.as_contract(&cid, || setup_token(&env, 401, &seller));
    client
        .try_create_listing(&listing_req(&seller, 401, 1000, &asset))
        .unwrap();

    // Self-purchase rejected.
    assert_eq!(
        client.try_buy_listing(&seller, &401, &asset, &1000),
        Err(Ok(Error::SelfTransferNotAllowed))
    );
    // Wrong asset / wrong amount rejected.
    let other_asset = Address::generate(&env);
    env.as_contract(&cid, || {
        crate::payment_currency::add_currency(&env, other_asset.clone()).unwrap();
    });
    assert_eq!(
        client.try_buy_listing(&buyer, &401, &other_asset, &1000),
        Err(Ok(Error::PaymentAssetMismatch))
    );
    assert_eq!(
        client.try_buy_listing(&buyer, &401, &asset, &999),
        Err(Ok(Error::IncorrectPaymentAmount))
    );
    // Unsupported asset rejected even when it matches the listing.
    env.as_contract(&cid, || {
        crate::payment_currency::remove_currency(&env, &asset).unwrap();
    });
    assert_eq!(
        client.try_buy_listing(&buyer, &401, &asset, &1000),
        Err(Ok(Error::UnsupportedAsset))
    );
    env.as_contract(&cid, || {
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
    });
    // Paused / frozen / blacklisted buyers blocked.
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_buy_listing(&buyer, &401, &asset, &1000),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 401));
    assert_eq!(
        client.try_buy_listing(&buyer, &401, &asset, &1000),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 401));
    env.as_contract(&cid, || blacklist::add_wallet(&env, &buyer));
    assert_eq!(
        client.try_buy_listing(&buyer, &401, &asset, &1000),
        Err(Ok(Error::InvalidAddress))
    );
    env.as_contract(&cid, || blacklist::remove_wallet(&env, &buyer));

    // Validator-level success path: pre-conditions hold for a well-formed
    // marketplace listing (full settlement needs live asset contracts, so the
    // entry-point success path is covered by the guard checks above).
    env.as_contract(&cid, || {
        let listing = crate::marketplace::types::Listing {
            token_id: 401,
            seller: seller.clone(),
            price: 1000,
            payment_asset: asset.clone(),
            expires_at: 0,
            status: crate::marketplace::types::ListingStatus::Active,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };
        assert!(crate::marketplace::purchase_validator::validate_purchase(
            &env, &buyer, &listing, &asset, 1000
        )
        .is_ok());
    });
}

// ── #1080: royalty configuration guards ───────────────────────────────────────
#[test]
fn royalty_configuration_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    init_contract(&env, &cid, &admin);
    let client = ClipsNftContractClient::new(&env, &cid);

    let creator = Address::generate(&env);
    env.as_contract(&cid, || {
        setup_token(&env, 501, &admin);
        crate::creator_storage::set_creator(&env, 501, &creator);
        crate::token_storage::set_royalty(&env, 501, &royalty_with(&env, &creator, 100));
    });
    let valid = royalty_with(&env, &creator, 200);

    // set_royalty: admin-only, pause-aware, freeze-aware.
    let stranger = Address::generate(&env);
    assert_eq!(
        client.try_set_royalty(&stranger, &501, &valid),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_set_royalty(&admin, &501, &valid),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 501));
    assert_eq!(
        client.try_set_royalty(&admin, &501, &valid),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 501));
    env.as_contract(&cid, || {
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyFrozen(501), &true);
    });
    assert_eq!(
        client.try_set_royalty(&admin, &501, &valid),
        Err(Ok(Error::RoyaltyFrozen))
    );
    env.as_contract(&cid, || {
        env.storage()
            .persistent()
            .remove(&DataKey::RoyaltyFrozen(501));
    });
    assert!(client.try_set_royalty(&admin, &501, &valid).is_ok());

    // update_royalty: admin / creator / owner pass, strangers fail.
    let owner = Address::generate(&env);
    env.as_contract(&cid, || {
        setup_token(&env, 502, &owner);
        crate::creator_storage::set_creator(&env, 502, &creator);
        crate::token_storage::set_royalty(&env, 502, &royalty_with(&env, &creator, 100));
    });
    let update = royalty_with(&env, &owner, 300);
    assert!(client.try_update_royalty(&admin, &502, &update).is_ok());
    assert!(client
        .try_update_royalty(&creator, &502, &update)
        .is_ok());
    assert!(client.try_update_royalty(&owner, &502, &update).is_ok());
    assert_eq!(
        client.try_update_royalty(&stranger, &502, &update),
        Err(Ok(Error::UnauthorizedConfigurationUpdate))
    );
    // Paused / frozen / blacklisted callers blocked.
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_update_royalty(&owner, &502, &update),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    env.as_contract(&cid, || {
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyFrozen(502), &true);
    });
    assert_eq!(
        client.try_update_royalty(&owner, &502, &update),
        Err(Ok(Error::RoyaltyFrozen))
    );
    env.as_contract(&cid, || {
        env.storage()
            .persistent()
            .remove(&DataKey::RoyaltyFrozen(502));
    });
    env.as_contract(&cid, || blacklist::add_wallet(&env, &owner));
    assert_eq!(
        client.try_update_royalty(&owner, &502, &update),
        Err(Ok(Error::InvalidAddress))
    );
    env.as_contract(&cid, || blacklist::remove_wallet(&env, &owner));
}

// ── #1080: royalty payment guards ─────────────────────────────────────────────
#[test]
fn royalty_payment_guards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let cid = env.register(ClipsNftContract, ());
    init_contract(&env, &cid, &admin);
    let client = ClipsNftContractClient::new(&env, &cid);

    let recipient = Address::generate(&env);
    let payer = Address::generate(&env);
    env.as_contract(&cid, || {
        setup_token(&env, 601, &admin);
        crate::token_storage::set_royalty(&env, 601, &royalty_with(&env, &recipient, 100));
    });

    // Unknown tokens rejected.
    assert_eq!(
        client.try_pay_royalty(&payer, &9999, &1000),
        Err(Ok(Error::TokenNotFound))
    );
    // Paused contract blocks payments.
    env.as_contract(&cid, || pause_state::save_pause_state(&env, true));
    assert_eq!(
        client.try_pay_royalty(&payer, &601, &1000),
        Err(Ok(Error::ContractPaused))
    );
    env.as_contract(&cid, || pause_state::save_pause_state(&env, false));
    // Emergency toggle blocks payments.
    env.as_contract(&cid, || {
        crate::royalty_emergency::set_payments_disabled(&env, &admin, true).unwrap();
    });
    assert_eq!(
        client.try_pay_royalty(&payer, &601, &1000),
        Err(Ok(Error::RoyaltyPaymentsDisabled))
    );
    env.as_contract(&cid, || {
        crate::royalty_emergency::set_payments_disabled(&env, &admin, false).unwrap();
    });
    // Frozen tokens and blacklisted payers blocked.
    env.as_contract(&cid, || frozen_token::freeze_token(&env, 601));
    assert_eq!(
        client.try_pay_royalty(&payer, &601, &1000),
        Err(Ok(Error::Unauthorized))
    );
    env.as_contract(&cid, || frozen_token::unfreeze_token(&env, 601));
    env.as_contract(&cid, || blacklist::add_wallet(&env, &payer));
    assert_eq!(
        client.try_pay_royalty(&payer, &601, &1000),
        Err(Ok(Error::InvalidAddress))
    );
    env.as_contract(&cid, || blacklist::remove_wallet(&env, &payer));

    // freeze_royalty: authorized identities pass once, then config is locked.
    let owner = Address::generate(&env);
    env.as_contract(&cid, || {
        setup_token(&env, 602, &owner);
        crate::creator_storage::set_creator(&env, 602, &owner);
        crate::token_storage::set_royalty(&env, 602, &royalty_with(&env, &owner, 100));
    });
    assert!(client.try_freeze_royalty(&owner, &602).is_ok());
    assert_eq!(
        client.try_freeze_royalty(&owner, &602),
        Err(Ok(Error::RoyaltyFrozen))
    );
    assert_eq!(
        client.try_update_royalty(&owner, &602, &royalty_with(&env, &owner, 200)),
        Err(Ok(Error::RoyaltyFrozen))
    );
}
