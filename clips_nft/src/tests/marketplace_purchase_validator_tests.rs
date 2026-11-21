//! Tests for the marketplace purchase validator.
//!
//! Covers both issues:
//! - "Validate Buyer Payment Balance" — buyer balance check, rejection on
//!   insufficient balance, and descriptive `InsufficientBalance` error.
//! - "Create Purchase Validator" — full validation pipeline: listing exists,
//!   active status, not expired, buyer address, payment asset, payment amount,
//!   and NFT ownership.

#[cfg(test)]
mod tests {
    use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Map};

    // ── Minimal SEP-41 mock token ─────────────────────────────────────────────

    /// A lightweight token contract that implements just enough of SEP-41 to
    /// satisfy `token::Client::balance` and `token::Client::transfer` calls.
    #[contract]
    struct MockToken;

    #[contractimpl]
    impl MockToken {
        /// Credit `amount` to `to` without authorization requirements (for test setup).
        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = symbol_short!("bal");
            let mut balances: Map<Address, i128> = env
                .storage()
                .instance()
                .get(&key)
                .unwrap_or_else(|| Map::new(&env));
            let cur = balances.get(to.clone()).unwrap_or(0);
            balances.set(to.clone(), cur + amount);
            env.storage().instance().set(&key, &balances);
        }

        /// Transfer `amount` from `from` to `to`, enforcing `from` auth.
        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            from.require_auth();
            let key = symbol_short!("bal");
            let mut balances: Map<Address, i128> = env
                .storage()
                .instance()
                .get(&key)
                .unwrap_or_else(|| Map::new(&env));
            let from_bal = balances.get(from.clone()).unwrap_or(0);
            if from_bal < amount {
                panic!("insufficient balance");
            }
            balances.set(from.clone(), from_bal - amount);
            let to_bal = balances.get(to.clone()).unwrap_or(0);
            balances.set(to.clone(), to_bal + amount);
            env.storage().instance().set(&key, &balances);
        }

        /// Return the current balance of `id`.
        pub fn balance(env: Env, id: Address) -> i128 {
            let key = symbol_short!("bal");
            env.storage()
                .instance()
                .get::<_, Map<Address, i128>>(&key)
                .unwrap_or_else(|| Map::new(&env))
                .get(id)
                .unwrap_or(0)
        }
    }

    // ── Test helpers ──────────────────────────────────────────────────────────

    use crate::{
        marketplace::{
            listing_storage,
            purchase_validator::{
                validate_purchase, validate_purchase_for_token, validate_purchase_request,
            },
            types::{Listing, ListingStatus},
        },
        pause_state::save_pause_state,
        payment_currency,
        purchase_request::PurchaseRequest,
        token_owner_storage,
        types::Error,
        AtomicMintContract,
    };

    /// Register the main contract so storage calls compile.
    fn register_contract(env: &Env) -> Address {
        env.register(AtomicMintContract, ())
    }

    /// Register a fresh `MockToken` contract and return its address.
    fn register_mock_token(env: &Env) -> Address {
        env.register(MockToken, ())
    }

    /// Assign `owner` as the on-chain owner of `token_id`.
    fn setup_token_owner(env: &Env, token_id: u32, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    /// Register a supported currency inside the contract context.
    fn register_currency(env: &Env, asset: Address) {
        payment_currency::add_currency(env, asset).unwrap();
    }

    /// Mint `amount` of the mock token to `recipient`.
    fn fund_buyer(env: &Env, token_addr: &Address, recipient: &Address, amount: i128) {
        MockTokenClient::new(env, token_addr).mint(recipient.clone(), amount);
    }

    /// Build a default active listing for `token_id`.
    fn make_listing(
        seller: &Address,
        asset: &Address,
        token_id: u32,
        price: i128,
        expires_at: u64,
        status: ListingStatus,
    ) -> Listing {
        Listing {
            token_id,
            seller: seller.clone(),
            price,
            payment_asset: asset.clone(),
            expires_at,
            status,
            created_at: 0,
            buyer: None,
            sold_at: None,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Issue: Validate Buyer Payment Balance
    // ═══════════════════════════════════════════════════════════════════════════

    /// Happy path: buyer has exactly enough balance.
    #[test]
    fn buyer_with_exact_balance_passes() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 1, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            let listing = make_listing(&seller, &asset, 1, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert!(validate_purchase(&env, &buyer, &listing, &asset, 1000).is_ok());
        });
    }

    /// Happy path: buyer has more than enough balance.
    #[test]
    fn buyer_with_surplus_balance_passes() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 2, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 5_000);

            let listing = make_listing(&seller, &asset, 2, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert!(validate_purchase(&env, &buyer, &listing, &asset, 1000).is_ok());
        });
    }

    /// Rejection criterion: buyer balance is zero.
    #[test]
    fn buyer_with_zero_balance_rejected_with_insufficient_balance() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 3, &seller);
            register_currency(&env, asset.clone());
            // buyer receives no funds — balance stays at 0

            let listing = make_listing(&seller, &asset, 3, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::InsufficientBalance)
            );
        });
    }

    /// Rejection criterion: buyer balance is positive but less than the price.
    #[test]
    fn buyer_with_partial_balance_rejected_with_insufficient_balance() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 4, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 999); // one short

            let listing = make_listing(&seller, &asset, 4, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::InsufficientBalance)
            );
        });
    }

    /// The error returned for insufficient balance must be `InsufficientBalance`
    /// (not a generic error), satisfying the "return descriptive error" criterion.
    #[test]
    fn insufficient_balance_returns_correct_error_variant() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 5, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 0);

            let listing = make_listing(&seller, &asset, 5, 500, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            let err = validate_purchase(&env, &buyer, &listing, &asset, 500).unwrap_err();
            assert_eq!(err, Error::InsufficientBalance);
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Issue: Create Purchase Validator — full pipeline
    // ═══════════════════════════════════════════════════════════════════════════

    // ── 1. Contract paused ────────────────────────────────────────────────────

    #[test]
    fn rejected_when_contract_is_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 10, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);
            save_pause_state(&env, true);

            let listing = make_listing(&seller, &asset, 10, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::ContractPaused)
            );
        });
    }

    // ── 2. Self-purchase prevention ────────────────────────────────────────────

    #[test]
    fn rejected_when_buyer_is_seller() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 11, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &seller, 1000);

            let listing = make_listing(&seller, &asset, 11, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &seller, &listing, &asset, 1000),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    // ── 3. Listing active status ──────────────────────────────────────────────

    #[test]
    fn rejected_when_listing_is_sold() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 12, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            let listing = make_listing(&seller, &asset, 12, 1000, 0, ListingStatus::Sold);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::ListingAlreadySold)
            );
        });
    }

    #[test]
    fn rejected_when_listing_is_cancelled() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 13, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            let listing = make_listing(&seller, &asset, 13, 1000, 0, ListingStatus::Cancelled);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::ListingNotActive)
            );
        });
    }

    #[test]
    fn rejected_when_listing_status_is_expired() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 14, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            let listing = make_listing(&seller, &asset, 14, 1000, 0, ListingStatus::Expired);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::ListingExpired)
            );
        });
    }

    // ── 4. Listing expiry timestamp ────────────────────────────────────────────

    #[test]
    fn rejected_when_listing_expiry_timestamp_has_passed() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 15, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            // expires_at = 1 means it expired at ledger time 1, current time = 0
            // but ledger().timestamp() is 0 here so we use expires_at = 0+1=1
            // which is <= now (0 in test). Actually the check is expires_at <= now,
            // so expires_at = 1 with now = 0: 1 <= 0 is false → not expired.
            // Use a very small expires_at to trigger expiry in tests:
            let listing =
                make_listing(&seller, &asset, 15, 1000, 1, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            // In the test env ledger timestamp starts at 0. expires_at=1 means
            // the listing has NOT yet expired (1 > 0). Advance ledger time.
            env.ledger().with_mut(|li| li.timestamp = 2);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::ListingExpired)
            );
        });
    }

    #[test]
    fn listing_with_zero_expiry_never_expires() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 16, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            // expires_at = 0 means never expires
            let listing = make_listing(&seller, &asset, 16, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            // Advance time significantly — should still be valid
            env.ledger().with_mut(|li| li.timestamp = 9_999_999);

            assert!(validate_purchase(&env, &buyer, &listing, &asset, 1000).is_ok());
        });
    }

    // ── 5. Payment asset validation ────────────────────────────────────────────

    #[test]
    fn rejected_when_payment_asset_mismatches_listing() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let listed_asset = register_mock_token(&env);
            let wrong_asset = register_mock_token(&env);

            setup_token_owner(&env, 17, &seller);
            register_currency(&env, listed_asset.clone());
            register_currency(&env, wrong_asset.clone());
            fund_buyer(&env, &wrong_asset, &buyer, 1000);

            let listing = make_listing(&seller, &listed_asset, 17, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &wrong_asset, 1000),
                Err(Error::PaymentAssetMismatch)
            );
        });
    }

    #[test]
    fn rejected_when_payment_asset_not_supported() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 18, &seller);
            // deliberately do NOT register the currency

            let listing = make_listing(&seller, &asset, 18, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::UnsupportedAsset)
            );
        });
    }

    // ── 6. Payment amount validation ───────────────────────────────────────────

    #[test]
    fn rejected_when_amount_less_than_listing_price() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 19, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 5000);

            let listing = make_listing(&seller, &asset, 19, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 999),
                Err(Error::IncorrectPaymentAmount)
            );
        });
    }

    #[test]
    fn rejected_when_amount_greater_than_listing_price() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 20, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 5000);

            let listing = make_listing(&seller, &asset, 20, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1001),
                Err(Error::IncorrectPaymentAmount)
            );
        });
    }

    // ── 7. NFT ownership guard ─────────────────────────────────────────────────

    #[test]
    fn rejected_when_seller_no_longer_owns_nft() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let actual_owner = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            // Token was transferred to `actual_owner` after listing
            setup_token_owner(&env, 21, &actual_owner);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 1000);

            // Listing still references the original seller
            let listing = make_listing(&seller, &asset, 21, 1000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            assert_eq!(
                validate_purchase(&env, &buyer, &listing, &asset, 1000),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── 8. Listing not found ───────────────────────────────────────────────────

    #[test]
    fn validate_purchase_for_token_returns_not_found_for_missing_listing() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            // No listing is created for token 99
            assert_eq!(
                validate_purchase_for_token(&env, 99, &buyer, &asset, 1000),
                Err(Error::ListingNotFound)
            );
        });
    }

    // ── 9. Convenience wrapper: validate_purchase_for_token ───────────────────

    #[test]
    fn validate_purchase_for_token_succeeds_on_valid_input() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 30, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 2000);

            let listing = make_listing(&seller, &asset, 30, 2000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            let result = validate_purchase_for_token(&env, 30, &buyer, &asset, 2000);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().token_id, 30);
        });
    }

    // ── 10. Convenience wrapper: validate_purchase_request ────────────────────

    #[test]
    fn validate_purchase_request_succeeds_on_valid_input() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 31, &seller);
            register_currency(&env, asset.clone());
            fund_buyer(&env, &asset, &buyer, 3000);

            let listing = make_listing(&seller, &asset, 31, 3000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            let req = PurchaseRequest {
                listing_id: 31,
                buyer: buyer.clone(),
                payment_asset: asset.clone(),
                max_price: 3000,
            };

            let result = validate_purchase_request(&env, &buyer, &req);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().token_id, 31);
        });
    }

    #[test]
    fn validate_purchase_request_rejects_insufficient_balance() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = register_contract(&env);

        env.as_contract(&cid, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = register_mock_token(&env);

            setup_token_owner(&env, 32, &seller);
            register_currency(&env, asset.clone());
            // buyer has only 1000 but price is 3000
            fund_buyer(&env, &asset, &buyer, 1000);

            let listing = make_listing(&seller, &asset, 32, 3000, 0, ListingStatus::Active);
            listing_storage::save_listing(&env, &listing);

            let req = PurchaseRequest {
                listing_id: 32,
                buyer: buyer.clone(),
                payment_asset: asset.clone(),
                max_price: 3000,
            };

            assert_eq!(
                validate_purchase_request(&env, &buyer, &req),
                Err(Error::InsufficientBalance)
            );
        });
    }
}
