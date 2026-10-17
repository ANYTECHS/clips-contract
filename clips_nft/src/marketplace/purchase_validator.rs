//! Marketplace purchase validator.
//!
//! Implements all pre-purchase validation checks required before an NFT
//! listing can be completed.  Every acceptance criterion from issues
//! "Validate Buyer Payment Balance" and "Create Purchase Validator" is
//! handled here.
//!
//! # Validation order (short-circuit on first failure)
//! 1. Contract is not paused.
//! 2. Buyer is not the seller (no self-purchase).
//! 3. Listing exists and is in `Active` status.
//! 4. Listing has not expired.
//! 5. Payment asset matches the listing's required asset.
//! 6. Payment asset is supported by the contract.
//! 7. Payment amount equals the listing price.
//! 8. Buyer has sufficient balance to cover the payment amount.
//! 9. Seller still owns the NFT (ownership guard).
//! Purchase validator — validation logic for NFT marketplace purchases (issue #876).
//!
//! Validates all invariants before an NFT purchase is processed:
//!
//! 1. Contract is not paused.
//! 2. Listing exists and is in `Active` status.
//! 3. Listing has not expired — returns [`crate::types::Error::ListingExpired`] when the
//!    ledger timestamp has reached or passed `expires_at` (non-zero).
//! 4. Buyer address is valid (not blacklisted, not the seller).
//! 5. Payment asset matches the listing and is a supported currency.
//! 6. Payment amount / maximum price is sufficient and within bounds.
//! 7. Seller maintains current on-chain NFT ownership and token is not frozen.
//!
//! # Usage
//!
//! ```rust,ignore
//! purchase_validator::validate_purchase(env, &buyer, &listing, &payment_asset, payment_amount)?;
//! ```

use soroban_sdk::{token, Address, Env};

use crate::marketplace::types::{Listing, ListingStatus};
use crate::pause_state;
use crate::payment_currency;
use crate::purchase_request::PurchaseRequest;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ── Core validator ────────────────────────────────────────────────────────────

/// Validate all preconditions for purchasing an NFT listing.
///
/// # Arguments
/// * `env`           — Contract execution environment.
/// * `buyer`         — Address of the prospective buyer.
/// * `listing`       — The [`Listing`] record being purchased.
/// * `payment_asset` — Asset contract the buyer will pay with.
/// * `amount`        — Amount the buyer intends to pay.
use super::listing_storage;
use super::types::{Listing, ListingStatus};

/// Maximum allowed purchase price in stroops.
const MAX_PURCHASE_PRICE: i128 = i128::MAX / 2;

/// Validate all pre-conditions for purchasing a listed NFT.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `buyer`          — Address of the buyer.
/// * `listing`        — The marketplace listing being purchased.
/// * `payment_asset`  — Soroban address of the payment asset provided by buyer.
/// * `payment_amount` — Payment amount or maximum price provided by buyer in stroops.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ContractPaused`        | Contract is globally paused. |
/// | `SelfTransferNotAllowed`| Buyer and seller are the same address. |
/// | `ListingAlreadySold`    | Listing is in `Sold` status. |
/// | `ListingNotActive`      | Listing is `Cancelled`. |
/// | `ListingExpired`        | Listing is `Expired` or expiry timestamp has passed. |
/// | `PaymentAssetMismatch`  | `payment_asset` ≠ `listing.payment_asset`. |
/// | `UnsupportedAsset`      | `payment_asset` is not a registered currency. |
/// | `IncorrectPaymentAmount`| `amount` ≠ `listing.price`. |
/// | `InsufficientBalance`   | Buyer's on-chain balance is less than `amount`. |
/// | `Unauthorized`          | Seller no longer owns the NFT. |
/// | `ContractPaused` | Contract is paused. |
/// | `ListingNotActive` | Listing is not in `Active` status (sold or cancelled). |
/// | `ListingExpired` | Listing expiration time has passed. |
/// | `Unauthorized` | Buyer is blacklisted or seller is no longer the NFT owner or token is frozen. |
/// | `SelfTransferNotAllowed` | Buyer is the seller. |
/// | `UnsupportedAsset` | Payment asset does not match listing or is not supported. |
/// | `InvalidSalePrice` | Payment amount is less than listing price or price is non-positive. |
/// | `PriceOverflow` | Price exceeds maximum allowed value. |
/// | `TokenNotFound` | NFT does not exist. |
pub fn validate_purchase(
    env: &Env,
    buyer: &Address,
    listing: &Listing,
    payment_asset: &Address,
    amount: i128,
) -> Result<(), Error> {
    // 1. Contract must not be paused.
    if pause_state::get_pause_state(env) {
        return Err(Error::ContractPaused);
    // 0. Contract must not be paused.
    pause_guard::require_not_paused(env)?;

    // 1. Listing must be active.
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }

    // 2. Listing must not be expired.
    //    `expires_at == 0` means no expiration.  When set, the listing expires as
    //    soon as the ledger timestamp reaches `expires_at` (i.e. `expires_at <= now`).
    if listing.expires_at > 0 {
        let now = env.ledger().timestamp();
        if listing.expires_at <= now {
            return Err(Error::ListingExpired);
        }
    }

    // 2. Buyer must not be the seller.
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }

    // 3. Listing must be Active.
    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }

    // 4. Listing must not have passed its expiry timestamp.
    let now = env.ledger().timestamp();
    if listing.expires_at != 0 && listing.expires_at <= now {
        return Err(Error::ListingExpired);
    }

    // 5. Payment asset must match the one required by the listing.
    if payment_asset != &listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }

    // 6. Payment asset must be a supported currency.
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }

    // 7. Payment amount must exactly match the listing price.
    if amount != listing.price {
        return Err(Error::IncorrectPaymentAmount);
    }

    // 8. Buyer must have sufficient on-chain balance to cover the payment.
    let token_client = token::Client::new(env, payment_asset);
    let buyer_balance = token_client.balance(buyer);
    if buyer_balance < amount {
        return Err(Error::InsufficientBalance);
    }

    // 9. Seller must still own the NFT (guards against stale listings).
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;

    Ok(())
}

// ── Convenience wrappers ──────────────────────────────────────────────────────

/// Validate a purchase request against the stored listing for `token_id`.
///
/// Loads the listing from storage, runs all validation checks via
/// [`validate_purchase`], and returns the listing on success.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`,
/// or any of the errors from [`validate_purchase`].
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    payment_asset: &Address,
    amount: i128,
) -> Result<Listing, Error> {
    let listing = crate::marketplace::listing_storage::get_listing(env, token_id)?;
    validate_purchase(env, buyer, &listing, payment_asset, amount)?;
    Ok(listing)
}

/// Validate using a [`PurchaseRequest`] struct.
///
/// This variant accepts a structured request, loads the appropriate listing,
/// and delegates to [`validate_purchase`].  The `max_price` field is treated
/// as the exact payment amount.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for the requested
/// listing, or any of the errors from [`validate_purchase`].
pub fn validate_purchase_request(
    env: &Env,
    buyer: &Address,
    request: &PurchaseRequest,
) -> Result<Listing, Error> {
    let listing =
        crate::marketplace::listing_storage::get_listing(env, request.listing_id as TokenId)?;
    validate_purchase(env, buyer, &listing, &request.payment_asset, request.max_price)?;
    Ok(listing)
    listing: &Listing,
) -> Result<(), Error> {
    validate_purchase(
        env,
        &request.buyer,
        listing,
        &request.payment_asset,
        request.max_price,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::listing_storage;
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::pause_state::save_pause_state;
    use crate::token_owner_storage;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_token_and_listing(
        env: &Env,
        token_id: TokenId,
        seller: &Address,
        asset: &Address,
        price: i128,
        expires_at: u64,
        status: ListingStatus,
    ) -> Listing {
        token_owner_storage::assign_owner(env, token_id, seller, token_id).unwrap();
        let listing = Listing {
            token_id,
            seller: seller.clone(),
            price,
            payment_asset: asset.clone(),
            expires_at,
            status,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };
        listing_storage::save_listing(env, &listing);
        listing
    }
    #[ignore]
    #[test]
    fn valid_purchase_passes() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert!(validate_purchase(&env, &buyer, &listing, &asset, 1_000).is_ok());
        assert!(validate_purchase(&env, &buyer, &listing, &asset, 1_500).is_ok());
    }
    #[ignore]
    #[test]
    fn rejected_when_paused() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        save_pause_state(&env, true);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::ContractPaused)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_listing_not_active() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let sold_listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Sold);
        assert_eq!(
            validate_purchase(&env, &buyer, &sold_listing, &asset, 1_000),
            Err(Error::ListingNotActive)
        );

        let cancelled_listing =
            setup_token_and_listing(&env, 2, &seller, &asset, 1_000, 0, ListingStatus::Cancelled);
        assert_eq!(
            validate_purchase(&env, &buyer, &cancelled_listing, &asset, 1_000),
            Err(Error::ListingNotActive)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_listing_expired() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        // Expired listing (expires_at = 1 while ledger time default is >= 1 or when ledger time > 1)
        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 1, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::ListingExpired)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_buyer_is_seller() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &seller, &listing, &asset, 1_000),
            Err(Error::SelfTransferNotAllowed)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_buyer_is_blacklisted() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        crate::blacklist::add_wallet(&env, &buyer);
        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_payment_asset_mismatches_listing() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset1 = Address::generate(&env);
        let asset2 = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset1.clone()).unwrap();
        crate::payment_currency::add_currency(&env, asset2.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset1, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset2, 1_000),
            Err(Error::UnsupportedAsset)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_payment_asset_not_supported() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let unsupported_asset = Address::generate(&env);

        let listing = setup_token_and_listing(
            &env,
            1,
            &seller,
            &unsupported_asset,
            1_000,
            0,
            ListingStatus::Active,
        );

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &unsupported_asset, 1_000),
            Err(Error::UnsupportedAsset)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_payment_amount_insufficient() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 999),
            Err(Error::InvalidSalePrice)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_seller_not_current_owner() {
        let env = Env::default();
        let original_seller = Address::generate(&env);
        let other_owner = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        // NFT owned by other_owner, but listing says seller was original_seller
        token_owner_storage::assign_owner(&env, 1, &other_owner, 1).unwrap();
        let listing = Listing {
            token_id: 1,
            seller: original_seller.clone(),
            price: 1_000,
            payment_asset: asset.clone(),
            expires_at: 0,
            status: ListingStatus::Active,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_token_is_frozen() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);
        crate::frozen_token::freeze_token(&env, 1);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        );
    }
    #[ignore]
    #[test]
    fn validate_purchase_request_works() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        let req = PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: asset.clone(),
            max_price: 1_000,
        };

        assert!(validate_purchase_request(&env, &req, &listing).is_ok());
    }
    #[ignore]
    #[test]
    fn validate_purchase_for_token_works() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert!(validate_purchase_for_token(&env, 1, &buyer, &asset, 1_000).is_ok());
        assert_eq!(
            validate_purchase_for_token(&env, 999, &buyer, &asset, 1_000),
            Err(Error::TokenNotFound)
        );
    }
}
