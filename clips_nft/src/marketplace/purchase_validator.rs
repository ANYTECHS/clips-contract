//! Purchase validator — validation logic for NFT marketplace purchases (issue #876).
//!
//! Validates all invariants before an NFT purchase is processed:
//!
//! 1. Contract is not paused.
//! 2. Listing exists and is in `Active` status.
//! 3. Listing has not expired.
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

use soroban_sdk::{Address, Env};

use crate::pause_guard;
use crate::purchase_request::PurchaseRequest;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

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
/// | `ContractPaused` | Contract is paused. |
/// | `ListingNotActive` | Listing is not in `Active` status (sold or cancelled). |
/// | `OfferExpired` | Listing expiration time has passed. |
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
    payment_amount: i128,
) -> Result<(), Error> {
    // 0. Contract must not be paused.
    pause_guard::require_not_paused(env)?;

    // 1. Listing must be active.
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }

    // 2. Listing must not be expired.
    if listing.expires_at > 0 {
        let now = env.ledger().timestamp();
        if listing.expires_at <= now {
            return Err(Error::OfferExpired);
        }
    }

    // 3. Buyer address validation:
    // - Buyer cannot be the seller.
    if *buyer == listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    // - Buyer must not be blacklisted.
    if crate::blacklist::is_blacklisted(env, buyer) {
        return Err(Error::Unauthorized);
    }

    // 4. Payment asset validation:
    // - Must match listing payment asset.
    if *payment_asset != listing.payment_asset {
        return Err(Error::UnsupportedAsset);
    }
    // - Must be a supported currency.
    if !crate::payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }

    // 5. Payment amount validation:
    // - Listing price must be positive.
    if listing.price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    // - Price must not exceed maximum.
    if listing.price > MAX_PURCHASE_PRICE {
        return Err(Error::PriceOverflow);
    }
    // - Buyer's payment amount must be at least the listing price.
    if payment_amount < listing.price {
        return Err(Error::InvalidSalePrice);
    }

    // 6. NFT ownership validation:
    // - Seller must be current on-chain owner of the token.
    let current_owner = token_owner_storage::get_owner(env, listing.token_id)?;
    if current_owner != listing.seller {
        return Err(Error::Unauthorized);
    }
    // - Token must not be frozen (soulbound).
    if crate::frozen_token::is_frozen(env, listing.token_id) {
        return Err(Error::Unauthorized);
    }

    Ok(())
}

/// Validate purchase by loading the listing from persistent storage for `token_id`.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `token_id`       — NFT token ID to look up listing for.
/// * `buyer`          — Address of the buyer.
/// * `payment_asset`  — Address of the payment asset.
/// * `payment_amount` — Payment amount or maximum price provided by buyer.
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    payment_asset: &Address,
    payment_amount: i128,
) -> Result<Listing, Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    validate_purchase(env, buyer, &listing, payment_asset, payment_amount)?;
    Ok(listing)
}

/// Validate a [`PurchaseRequest`] against an existing [`Listing`].
///
/// # Arguments
/// * `env`     — Contract execution environment.
/// * `request` — The purchase request.
/// * `listing` — The marketplace listing being purchased.
pub fn validate_purchase_request(
    env: &Env,
    request: &PurchaseRequest,
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

    #[test]
    fn valid_purchase_passes() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert!(validate_purchase(&env, &buyer, &listing, &asset, 1_000).is_ok());
        assert!(validate_purchase(&env, &buyer, &listing, &asset, 1_500).is_ok());
    }

    #[test]
    fn rejected_when_paused() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        save_pause_state(&env, true);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::ContractPaused)
        );
    }

    #[test]
    fn rejected_when_listing_not_active() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let sold_listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Sold);
        assert_eq!(
            validate_purchase(&env, &buyer, &sold_listing, &asset, 1_000),
            Err(Error::ListingNotActive)
        );

        let cancelled_listing = setup_token_and_listing(&env, 2, &seller, &asset, 1_000, 0, ListingStatus::Cancelled);
        assert_eq!(
            validate_purchase(&env, &buyer, &cancelled_listing, &asset, 1_000),
            Err(Error::ListingNotActive)
        );
    }

    #[test]
    fn rejected_when_listing_expired() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        // Expired listing (expires_at = 100 while ledger time default is >= 100 or when ledger time > 100)
        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 1, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::OfferExpired)
        );
    }

    #[test]
    fn rejected_when_buyer_is_seller() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &seller, &listing, &asset, 1_000),
            Err(Error::SelfTransferNotAllowed)
        );
    }

    #[test]
    fn rejected_when_buyer_is_blacklisted() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        crate::blacklist::set_blacklisted(&env, &buyer, true);
        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn rejected_when_payment_asset_mismatches_listing() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset1 = Address::generate(&env);
        let asset2 = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset1.clone()).unwrap();
        crate::payment_currency::add_currency(&env, asset2.clone()).unwrap();

        let listing = setup_token_and_listing(&env, 1, &seller, &asset1, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset2, 1_000),
            Err(Error::UnsupportedAsset)
        );
    }

    #[test]
    fn rejected_when_payment_asset_not_supported() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let unsupported_asset = Address::generate(&env);

        let listing = setup_token_and_listing(&env, 1, &seller, &unsupported_asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &unsupported_asset, 1_000),
            Err(Error::UnsupportedAsset)
        );
    }

    #[test]
    fn rejected_when_payment_amount_insufficient() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 999),
            Err(Error::InvalidSalePrice)
        );
    }

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

    #[test]
    fn rejected_when_token_is_frozen() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);
        crate::frozen_token::freeze_token(&env, 1);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn validate_purchase_request_works() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        let req = PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: asset.clone(),
            max_price: 1_000,
        };

        assert!(validate_purchase_request(&env, &req, &listing).is_ok());
    }

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
