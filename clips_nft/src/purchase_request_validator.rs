//! Purchase request validator (issue #1202).
//!
//! Validates NFT purchase requests before marketplace settlement.
//!
//! A purchase request is the buyer's intent to buy a listed NFT at (or below)
//! a maximum price. Before the settlement path moves any funds, five
//! independent gates must pass:
//!
//! | Gate | Function | What it checks |
//! |------|----------|----------------|
//! | Listing existence | [`validate_listing_exists`] | A listing record is present for the token |
//! | Listing status | [`validate_listing_status`] | Listing is `Active` (not Sold/Cancelled/Expired) |
//! | Buyer | [`validate_purchase_buyer`] | Buyer is not the seller, not blacklisted, not the contract |
//! | Payment amount | [`validate_purchase_amount`] | Amount is positive, within bounds, and covers the listing price |
//! | Payment asset | [`validate_purchase_asset`] | Asset matches the listing and is a supported currency |
//!
//! # Acceptance criteria (issue #1202)
//!
//! 1. Validate listing existence — [`validate_listing_exists`].
//! 2. Validate listing status — [`validate_listing_status`].
//! 3. Validate buyer — [`validate_purchase_buyer`].
//! 4. Validate payment amount — [`validate_purchase_amount`].
//! 5. Validate payment asset — [`validate_purchase_asset`].
//!
//! # Errors
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::ListingNotFound`]    | No listing record exists for the token. |
//! | [`Error::ListingNotActive`]   | Listing is `Sold`, `Cancelled`, or `Expired`. |
//! | [`Error::SelfTransferNotAllowed`] | Buyer is the seller. |
//! | [`Error::Unauthorized`]       | Buyer (or seller) is blacklisted. |
//! | [`Error::InvalidAddress`]     | Buyer is the contract's own address. |
//! | [`Error::InvalidSalePrice`]   | Payment amount is zero/negative or below listing price. |
//! | [`Error::PriceOverflow`]      | Payment amount exceeds `MAX_PAYMENT_AMOUNT`. |
//! | [`Error::PaymentAssetMismatch`] | Payment asset differs from the listing's asset. |
//! | [`Error::UnsupportedAsset`]   | Payment asset is not a registered currency. |

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::marketplace::listing_storage;
use crate::marketplace::types::{Listing, ListingStatus};
use crate::payment_amount_validator::{validate_positive_within_bounds, MAX_PAYMENT_AMOUNT};
use crate::payment_currency;
use crate::purchase_request::PurchaseRequest;
use crate::types::{Error, TokenId};

// ── Listing existence gate ─────────────────────────────────────────────────────

/// Validate that a listing exists for `token_id` and return it.
///
/// # Errors
/// - [`Error::ListingNotFound`] — no listing record exists for `token_id`.
pub fn validate_listing_exists(env: &Env, token_id: TokenId) -> Result<Listing, Error> {
    listing_storage::get_listing(env, token_id).ok_or(Error::ListingNotFound)
}

// ── Listing status gate ────────────────────────────────────────────────────────

/// Validate that `listing` is in the `Active` status.
///
/// A listing that has already been sold, cancelled, or expired cannot be
/// purchased.
///
/// # Errors
/// - [`Error::ListingNotActive`] — `listing.status` is anything other than `Active`.
pub fn validate_listing_status(listing: &Listing) -> Result<(), Error> {
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    Ok(())
}

// ── Buyer gate ─────────────────────────────────────────────────────────────────

/// Validate `buyer` against the `listing` seller.
///
/// A buyer is eligible when:
/// 1. They are not the contract's own address.
/// 2. They are not blacklisted.
/// 3. They are not the listing seller (no self-purchase).
/// 4. The seller is not blacklisted (defensive: a blacklisted seller's
///    listing should never be purchasable).
///
/// # Errors
/// - [`Error::InvalidAddress`]           — `buyer` is the contract's address or blacklisted.
/// - [`Error::SelfTransferNotAllowed`]   — `buyer` is the listing seller.
/// - [`Error::Unauthorized`]             — seller is blacklisted.
pub fn validate_purchase_buyer(env: &Env, buyer: &Address, listing: &Listing) -> Result<(), Error> {
    if *buyer == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    if blacklist::is_blacklisted(env, buyer) {
        return Err(Error::InvalidAddress);
    }
    if *buyer == listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, &listing.seller) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

// ── Payment amount gate ───────────────────────────────────────────────────────

/// Validate the payment `amount` against the `listing` price.
///
/// The amount must be strictly positive, within [`MAX_PAYMENT_AMOUNT`], and at
/// least equal to the listing price. The upper bound guards downstream fee
/// arithmetic against `i128` overflow.
///
/// # Errors
/// - [`Error::InvalidSalePrice`] — `amount <= 0` or `amount < listing.price`.
/// - [`Error::PriceOverflow`]    — `amount > MAX_PAYMENT_AMOUNT`.
pub fn validate_purchase_amount(amount: i128, listing: &Listing) -> Result<(), Error> {
    validate_positive_within_bounds(amount)?;
    if amount < listing.price {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}

// ── Payment asset gate ────────────────────────────────────────────────────────

/// Validate `payment_asset` against the `listing` payment asset.
///
/// The asset must match the listing's required payment asset and must be a
/// supported currency.
///
/// # Errors
/// - [`Error::PaymentAssetMismatch`] — `payment_asset` differs from `listing.payment_asset`.
/// - [`Error::UnsupportedAsset`]    — `payment_asset` is not a registered currency.
pub fn validate_purchase_asset(
    env: &Env,
    payment_asset: &Address,
    listing: &Listing,
) -> Result<(), Error> {
    if *payment_asset != listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    Ok(())
}

// ── Full request validator ────────────────────────────────────────────────────

/// Validate a complete [`PurchaseRequest`] against its [`Listing`].
///
/// Runs every gate in order:
/// 1. Listing status ([`validate_listing_status`])
/// 2. Buyer ([`validate_purchase_buyer`])
/// 3. Payment amount ([`validate_purchase_amount`])
/// 4. Payment asset ([`validate_purchase_asset`])
///
/// Callers are expected to load the listing first via
/// [`validate_listing_exists`] (which returns the listing), then pass both the
/// request and the loaded listing here. This split lets the entry point reuse
/// the loaded listing for settlement without a second storage read.
///
/// # Errors
/// Aggregates the errors of every gate above; see their documentation.
pub fn validate_purchase_request(
    env: &Env,
    request: &PurchaseRequest,
    listing: &Listing,
) -> Result<(), Error> {
    validate_listing_status(listing)?;
    validate_purchase_buyer(env, &request.buyer, listing)?;
    validate_purchase_amount(request.max_price, listing)?;
    validate_purchase_asset(env, &request.payment_asset, listing)?;
    Ok(())
}

/// Load the listing for `token_id` and validate a [`PurchaseRequest`] against it.
///
/// Convenience wrapper that combines [`validate_listing_exists`] with
/// [`validate_purchase_request`] for callers that do not already hold the
/// listing.
///
/// # Errors
/// - [`Error::ListingNotFound`] — no listing record exists for `token_id`.
/// - Any error from [`validate_purchase_request`].
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    request: &PurchaseRequest,
) -> Result<Listing, Error> {
    let listing = validate_listing_exists(env, token_id)?;
    validate_purchase_request(env, request, &listing)?;
    Ok(listing)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::listing_storage;
    use crate::purchase_request::PurchaseRequest;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token_owner(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    fn setup_asset(env: &Env) -> Address {
        let asset = Address::generate(env);
        payment_currency::add_currency(env, asset.clone()).unwrap();
        asset
    }

    fn make_listing(
        seller: &Address,
        asset: &Address,
        token_id: TokenId,
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

    fn save_listing(env: &Env, listing: &Listing) {
        listing_storage::save_listing(env, listing);
    }

    fn make_request(
        buyer: &Address,
        asset: &Address,
        max_price: i128,
    ) -> PurchaseRequest {
        PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: asset.clone(),
            max_price,
        }
    }

    // ── validate_listing_exists ──────────────────────────────────────────────

    #[test]
    fn listing_exists_accepts_present_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_asset(env);
            setup_token_owner(env, 1, &seller);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            save_listing(env, &listing);
            let result = validate_listing_exists(env, 1).unwrap();
            assert_eq!(result.token_id, 1);
        });
    }

    #[test]
    fn listing_exists_rejects_missing_listing() {
        with_contract(|env| {
            assert_eq!(validate_listing_exists(env, 1), Err(Error::ListingNotFound));
        });
    }

    // ── validate_listing_status ──────────────────────────────────────────────

    #[test]
    fn listing_status_accepts_active() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert!(validate_listing_status(&listing).is_ok());
    }

    #[test]
    fn listing_status_rejects_sold() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Sold);
        assert_eq!(validate_listing_status(&listing), Err(Error::ListingNotActive));
    }

    #[test]
    fn listing_status_rejects_cancelled() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Cancelled);
        assert_eq!(validate_listing_status(&listing), Err(Error::ListingNotActive));
    }

    // ── validate_purchase_buyer ──────────────────────────────────────────────

    #[test]
    fn buyer_accepts_external_non_seller() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert!(validate_purchase_buyer(env, &buyer, &listing).is_ok());
        });
    }

    #[test]
    fn buyer_rejects_contract_self_address() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            let seller = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert_eq!(
                validate_purchase_buyer(env, &contract_addr, &listing),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn buyer_rejects_blacklisted_buyer() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            blacklist::add_wallet(env, &buyer);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert_eq!(
                validate_purchase_buyer(env, &buyer, &listing),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn buyer_rejects_self_purchase() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert_eq!(
                validate_purchase_buyer(env, &seller, &listing),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn buyer_rejects_blacklisted_seller() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            blacklist::add_wallet(env, &seller);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert_eq!(
                validate_purchase_buyer(env, &buyer, &listing),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── validate_purchase_amount ──────────────────────────────────────────────

    #[test]
    fn amount_accepts_exact_price() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert!(validate_purchase_amount(1_000, &listing).is_ok());
    }

    #[test]
    fn amount_accepts_above_price() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert!(validate_purchase_amount(1_500, &listing).is_ok());
    }

    #[test]
    fn amount_rejects_below_price() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert_eq!(
            validate_purchase_amount(999, &listing),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn amount_rejects_zero() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert_eq!(
            validate_purchase_amount(0, &listing),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn amount_rejects_negative() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert_eq!(
            validate_purchase_amount(-1, &listing),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn amount_rejects_above_max() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
        assert_eq!(
            validate_purchase_amount(MAX_PAYMENT_AMOUNT + 1, &listing),
            Err(Error::PriceOverflow)
        );
    }

    // ── validate_purchase_asset ──────────────────────────────────────────────

    #[test]
    fn asset_accepts_matching_supported() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert!(validate_purchase_asset(env, &asset, &listing).is_ok());
        });
    }

    #[test]
    fn asset_rejects_mismatch() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_asset(env);
            let other = Address::generate(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert_eq!(
                validate_purchase_asset(env, &other, &listing),
                Err(Error::PaymentAssetMismatch)
            );
        });
    }

    #[test]
    fn asset_rejects_unsupported() {
        with_contract(|env| {
            let seller = Address::generate(env);
            // listing asset is not registered as a supported currency
            let asset = Address::generate(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            assert_eq!(
                validate_purchase_asset(env, &asset, &listing),
                Err(Error::UnsupportedAsset)
            );
        });
    }

    // ── validate_purchase_request (full pipeline) ──────────────────────────────

    #[test]
    fn valid_purchase_request_passes() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            setup_token_owner(env, 1, &seller);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            save_listing(env, &listing);
            let req = make_request(&buyer, &asset, 1_000);
            assert!(validate_purchase_request(env, &req, &listing).is_ok());
        });
    }

    #[test]
    fn request_rejects_inactive_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Sold);
            let req = make_request(&buyer, &asset, 1_000);
            assert_eq!(
                validate_purchase_request(env, &req, &listing),
                Err(Error::ListingNotActive)
            );
        });
    }

    #[test]
    fn request_rejects_self_purchase() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            let req = make_request(&seller, &asset, 1_000);
            assert_eq!(
                validate_purchase_request(env, &req, &listing),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn request_rejects_insufficient_amount() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            let req = make_request(&buyer, &asset, 500);
            assert_eq!(
                validate_purchase_request(env, &req, &listing),
                Err(Error::InvalidSalePrice)
            );
        });
    }

    #[test]
    fn request_rejects_asset_mismatch() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            let other = Address::generate(env);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            let req = make_request(&buyer, &other, 1_000);
            assert_eq!(
                validate_purchase_request(env, &req, &listing),
                Err(Error::PaymentAssetMismatch)
            );
        });
    }

    // ── validate_purchase_for_token (convenience wrapper) ───────────────────────

    #[test]
    fn for_token_loads_and_validates() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            setup_token_owner(env, 1, &seller);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            save_listing(env, &listing);
            let req = make_request(&buyer, &asset, 1_000);
            let loaded = validate_purchase_for_token(env, 1, &req).unwrap();
            assert_eq!(loaded.token_id, 1);
        });
    }

    #[test]
    fn for_token_rejects_missing_listing() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            let req = make_request(&buyer, &asset, 1_000);
            assert_eq!(
                validate_purchase_for_token(env, 1, &req),
                Err(Error::ListingNotFound)
            );
        });
    }

    // ── Centralized-error integration ─────────────────────────────────────────

    #[test]
    fn purchase_errors_are_centralized_error_variants() {
        // Every gate returns crate::types::Error variants (not custom error
        // types), satisfying the "centralized errors" acceptance criterion.
        with_contract(|env| {
            let seller = Address::generate(env);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            setup_token_owner(env, 1, &seller);
            let listing = make_listing(&seller, &asset, 1, 1_000, 0, ListingStatus::Active);
            save_listing(env, &listing);

            // distinct failure → distinct centralized error
            let self_req = make_request(&seller, &asset, 1_000);
            assert_eq!(
                validate_purchase_request(env, &self_req, &listing),
                Err(Error::SelfTransferNotAllowed)
            );

            let low_req = make_request(&buyer, &asset, 500);
            assert_eq!(
                validate_purchase_request(env, &low_req, &listing),
                Err(Error::InvalidSalePrice)
            );

            let bad_asset_req = make_request(&buyer, &Address::generate(env), 1_000);
            assert_eq!(
                validate_purchase_request(env, &bad_asset_req, &listing),
                Err(Error::PaymentAssetMismatch)
            );
        });
    }
}
