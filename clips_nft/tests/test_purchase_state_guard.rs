//! Integration test suite for the Purchase State Guard (issue #1027).
//!
//! These tests exercise [`clips_nft::purchase_state_guard`] through a fully
//! registered Soroban contract environment to verify all purchase-state invariants:
//!
//! 1. Verify listing exists ([`Error::ListingNotFound`]).
//! 2. Verify listing is active ([`Error::ListingNotActive`]).
//! 3. Verify listing has not expired ([`Error::ListingExpired`]).
//! 4. Reject already-sold listings ([`Error::ListingAlreadySold`]).

#![cfg(test)]

use clips_nft::marketplace::listing_storage;
use clips_nft::marketplace::types::{Listing, ListingStatus};
use clips_nft::purchase_state_guard::{
    check_listing_active, check_listing_exists, check_listing_not_expired, check_listing_not_sold,
    get_purchasable_listing, require_purchasable, require_purchasable_listing,
};
use clips_nft::{AtomicMintContract, Error, TokenId};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env,
};

// ─── Test Harness ────────────────────────────────────────────────────────────

fn with_contract<F, R>(f: F) -> R
where
    F: FnOnce(&Env) -> R,
{
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AtomicMintContract, ());
    env.as_contract(&contract_id, || f(&env))
}

fn create_test_listing(
    env: &Env,
    token_id: TokenId,
    expires_at: u64,
    status: ListingStatus,
) -> Listing {
    let listing = Listing {
        token_id,
        seller: Address::generate(env),
        price: 500_000,
        payment_asset: Address::generate(env),
        expires_at,
        status,
        created_at: 100,
        buyer: None,
        sold_at: None,
    };
    listing_storage::save_listing(env, &listing);
    listing
}

// ─── Acceptance Criterion 1: Verify listing exists ───────────────────────────

#[test]
fn test_missing_listing_returns_listing_not_found() {
    with_contract(|env| {
        let non_existent_token_id: TokenId = 404;

        assert_eq!(
            require_purchasable(env, non_existent_token_id),
            Err(Error::ListingNotFound)
        );
        assert_eq!(
            get_purchasable_listing(env, non_existent_token_id),
            Err(Error::ListingNotFound)
        );
        assert_eq!(
            check_listing_exists(env, non_existent_token_id),
            Err(Error::ListingNotFound)
        );
    });
}

#[test]
fn test_existing_listing_is_found() {
    with_contract(|env| {
        let token_id: TokenId = 10;
        let created = create_test_listing(env, token_id, 0, ListingStatus::Active);

        let fetched = check_listing_exists(env, token_id).expect("should find listing");
        assert_eq!(fetched.token_id, created.token_id);
        assert_eq!(fetched.seller, created.seller);
        assert_eq!(fetched.price, created.price);
        assert_eq!(fetched.status, ListingStatus::Active);
    });
}

#[test]
fn test_removed_listing_fails_existence_check() {
    with_contract(|env| {
        let token_id: TokenId = 11;
        create_test_listing(env, token_id, 0, ListingStatus::Active);
        assert!(require_purchasable(env, token_id).is_ok());

        listing_storage::remove_listing(env, token_id);

        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingNotFound)
        );
        assert_eq!(
            check_listing_exists(env, token_id),
            Err(Error::ListingNotFound)
        );
    });
}

// ─── Acceptance Criterion 2: Verify listing is active ────────────────────────

#[test]
fn test_active_listing_passes_active_check() {
    with_contract(|env| {
        let token_id: TokenId = 20;
        let listing = create_test_listing(env, token_id, 0, ListingStatus::Active);

        assert!(check_listing_active(&listing).is_ok());
        assert!(require_purchasable_listing(env, &listing).is_ok());
        assert!(require_purchasable(env, token_id).is_ok());
    });
}

#[test]
fn test_cancelled_listing_rejected_with_listing_not_active() {
    with_contract(|env| {
        let token_id: TokenId = 21;
        let listing = create_test_listing(env, token_id, 0, ListingStatus::Cancelled);

        assert_eq!(check_listing_active(&listing), Err(Error::ListingNotActive));
        assert_eq!(
            require_purchasable_listing(env, &listing),
            Err(Error::ListingNotActive)
        );
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingNotActive)
        );
    });
}

#[test]
fn test_cancelled_via_status_update_is_rejected() {
    with_contract(|env| {
        let token_id: TokenId = 22;
        create_test_listing(env, token_id, 0, ListingStatus::Active);
        assert!(require_purchasable(env, token_id).is_ok());

        listing_storage::update_listing_status(env, token_id, ListingStatus::Cancelled).unwrap();

        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingNotActive)
        );
    });
}

// ─── Acceptance Criterion 3: Verify listing has not expired ──────────────────

#[test]
fn test_zero_expiration_never_expires() {
    with_contract(|env| {
        let token_id: TokenId = 30;
        env.ledger().set_timestamp(999_999_999);
        let listing = create_test_listing(env, token_id, 0, ListingStatus::Active);

        assert!(check_listing_not_expired(env, &listing).is_ok());
        assert!(require_purchasable_listing(env, &listing).is_ok());
        assert!(require_purchasable(env, token_id).is_ok());
    });
}

#[test]
fn test_future_expiration_passes() {
    with_contract(|env| {
        let token_id: TokenId = 31;
        env.ledger().set_timestamp(1_000);
        let listing = create_test_listing(env, token_id, 2_000, ListingStatus::Active);

        assert!(check_listing_not_expired(env, &listing).is_ok());
        assert!(require_purchasable_listing(env, &listing).is_ok());
        assert!(require_purchasable(env, token_id).is_ok());
    });
}

#[test]
fn test_expired_listing_rejected_with_listing_expired() {
    with_contract(|env| {
        let token_id: TokenId = 32;
        env.ledger().set_timestamp(2_000);
        let listing = create_test_listing(env, token_id, 1_500, ListingStatus::Active);

        assert_eq!(
            check_listing_not_expired(env, &listing),
            Err(Error::ListingExpired)
        );
        assert_eq!(
            require_purchasable_listing(env, &listing),
            Err(Error::ListingExpired)
        );
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingExpired)
        );
    });
}

#[test]
fn test_exact_expiration_timestamp_is_rejected() {
    with_contract(|env| {
        let token_id: TokenId = 33;
        env.ledger().set_timestamp(1_500);
        let listing = create_test_listing(env, token_id, 1_500, ListingStatus::Active);

        assert_eq!(
            check_listing_not_expired(env, &listing),
            Err(Error::ListingExpired)
        );
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingExpired)
        );
    });
}

#[test]
fn test_one_second_before_expiration_is_valid() {
    with_contract(|env| {
        let token_id: TokenId = 34;
        env.ledger().set_timestamp(1_499);
        let listing = create_test_listing(env, token_id, 1_500, ListingStatus::Active);

        assert!(check_listing_not_expired(env, &listing).is_ok());
        assert!(require_purchasable(env, token_id).is_ok());
    });
}

#[test]
fn test_ledger_advancement_triggers_expiration() {
    with_contract(|env| {
        let token_id: TokenId = 35;
        env.ledger().set_timestamp(1_000);
        create_test_listing(env, token_id, 1_500, ListingStatus::Active);

        // Before expiry: purchasable
        assert!(require_purchasable(env, token_id).is_ok());

        // Advance ledger time past expiration
        env.ledger().set_timestamp(1_501);

        // After expiry: rejected with ListingExpired
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingExpired)
        );
    });
}

// ─── Acceptance Criterion 4: Reject already-sold listings ─────────────────────

#[test]
fn test_sold_listing_rejected_with_listing_already_sold() {
    with_contract(|env| {
        let token_id: TokenId = 40;
        let listing = create_test_listing(env, token_id, 0, ListingStatus::Sold);

        assert_eq!(
            check_listing_not_sold(&listing),
            Err(Error::ListingAlreadySold)
        );
        assert_eq!(
            require_purchasable_listing(env, &listing),
            Err(Error::ListingAlreadySold)
        );
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingAlreadySold)
        );
    });
}

#[test]
fn test_transition_to_sold_via_mark_as_sold_rejects_subsequent_purchases() {
    with_contract(|env| {
        let token_id: TokenId = 41;
        create_test_listing(env, token_id, 0, ListingStatus::Active);
        assert!(require_purchasable(env, token_id).is_ok());

        let buyer = Address::generate(env);
        listing_storage::mark_as_sold(env, token_id, &buyer).unwrap();

        // Must reject subsequent purchase attempts with ListingAlreadySold
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingAlreadySold)
        );
    });
}

#[test]
fn test_sold_takes_precedence_over_expired() {
    with_contract(|env| {
        let token_id: TokenId = 42;
        env.ledger().set_timestamp(10_000);
        // Both expired (expires_at = 1000 < now = 10000) AND sold
        let listing = create_test_listing(env, token_id, 1_000, ListingStatus::Sold);

        assert_eq!(
            require_purchasable_listing(env, &listing),
            Err(Error::ListingAlreadySold)
        );
        assert_eq!(
            require_purchasable(env, token_id),
            Err(Error::ListingAlreadySold)
        );
    });
}

// ─── Composite & Multi-Token Invariants ───────────────────────────────────────

#[test]
fn test_get_purchasable_listing_returns_valid_data() {
    with_contract(|env| {
        let token_id: TokenId = 50;
        let listing = create_test_listing(env, token_id, 5_000, ListingStatus::Active);
        env.ledger().set_timestamp(1_000);

        let validated = get_purchasable_listing(env, token_id).unwrap();
        assert_eq!(validated.token_id, token_id);
        assert_eq!(validated.price, listing.price);
        assert_eq!(validated.seller, listing.seller);
        assert_eq!(validated.expires_at, 5_000);
        assert_eq!(validated.status, ListingStatus::Active);
    });
}

#[test]
fn test_multi_token_listing_state_isolation() {
    with_contract(|env| {
        env.ledger().set_timestamp(2_000);

        // Token 1: active, no expiration -> passes
        create_test_listing(env, 1, 0, ListingStatus::Active);
        // Token 2: active, future expiration -> passes
        create_test_listing(env, 2, 3_000, ListingStatus::Active);
        // Token 3: active, expired -> rejected
        create_test_listing(env, 3, 1_500, ListingStatus::Active);
        // Token 4: sold -> rejected
        create_test_listing(env, 4, 0, ListingStatus::Sold);
        // Token 5: cancelled -> rejected
        create_test_listing(env, 5, 0, ListingStatus::Cancelled);
        // Token 6: no listing -> rejected

        assert!(require_purchasable(env, 1).is_ok());
        assert!(require_purchasable(env, 2).is_ok());
        assert_eq!(require_purchasable(env, 3), Err(Error::ListingExpired));
        assert_eq!(require_purchasable(env, 4), Err(Error::ListingAlreadySold));
        assert_eq!(require_purchasable(env, 5), Err(Error::ListingNotActive));
        assert_eq!(require_purchasable(env, 6), Err(Error::ListingNotFound));
    });
}
