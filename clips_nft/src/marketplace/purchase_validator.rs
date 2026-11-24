//! Purchase validation for NFT marketplace listings.

use soroban_sdk::{token, Address, Env};

use super::listing_storage;
use super::types::{Listing, ListingStatus};
use crate::{
    blacklist, pause_guard, payment_currency, purchase_request::PurchaseRequest,
    token_owner_storage,
    types::{Error, TokenId},
};

/// Validate every precondition for a direct marketplace purchase.
pub fn validate_purchase(
    env: &Env,
    buyer: &Address,
    listing: &Listing,
    payment_asset: &Address,
    price: i128,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    if is_expired(env.ledger().timestamp(), listing.expires_at) {
        return Err(Error::OfferExpired);
    }
    if *buyer == listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, buyer) || blacklist::is_blacklisted(env, &listing.seller) {
        return Err(Error::Unauthorized);
    }
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;
    if *payment_asset != listing.payment_asset
        || !payment_currency::is_supported(env, payment_asset)
    {
        return Err(Error::UnsupportedAsset {
            asset: payment_asset.clone(),
        });
    }
    if price < listing.price {
        return Err(Error::InsufficientPayment {
            expected: listing.price,
            provided: price,
        });
    }
    Ok(())
}

/// Load the active listing for a token and validate the purchase.
/// Returns the listing so callers can settle against its terms.
    amount: i128,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;

    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }

    if listing.expires_at != 0 && listing.expires_at <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, buyer)
        || blacklist::is_blacklisted(env, &listing.seller)
    {
        return Err(Error::Unauthorized);
    }
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;
    if payment_asset != &listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if amount != listing.price {
        return Err(Error::IncorrectPaymentAmount);
    amount: i128,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;

    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }

    if listing.expires_at != 0 && listing.expires_at <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, buyer)
        || blacklist::is_blacklisted(env, &listing.seller)
    {
        return Err(Error::Unauthorized);
    }
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)
        .map_err(|_| Error::UnauthorizedOwnerError)?;
    if payment_asset != &listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if amount != listing.price {
        return Err(Error::IncorrectPaymentAmount);
    amount: i128,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;

    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }

    if listing.expires_at != 0 && listing.expires_at <= env.ledger().timestamp() {
        return Err(Error::ListingExpired);
    }
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if blacklist::is_blacklisted(env, buyer)
        || blacklist::is_blacklisted(env, &listing.seller)
    {
        return Err(Error::Unauthorized);
    }
    if payment_asset != &listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    if amount != listing.price {
        return Err(Error::IncorrectPaymentAmount);
    }
    if amount <= 0 {
        return Err(Error::InvalidSalePrice);
    }

    let buyer_balance = token::Client::new(env, payment_asset).balance(buyer);
    if buyer_balance < amount {
        return Err(Error::InsufficientBalance);
    }

    // 9. Seller must still own the NFT (guards against stale listings).
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)
        .map_err(|_| Error::UnauthorizedOwnerError)?;
    Ok(())
}

/// Load a listing and validate a direct purchase against it.
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    payment_asset: &Address,
    amount: i128,
) -> Result<Listing, Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    validate_purchase(env, buyer, &listing, payment_asset, amount)?;
    Ok(listing)
}

/// Validate a purchase request whose `max_price` is allowed to cover the listing.
pub fn validate_purchase_request(
    env: &Env,
    req: &PurchaseRequest,
    listing: &Listing,
) -> Result<(), Error> {
    pause_guard::require_not_paused(env)?;
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    if is_expired(env.ledger().timestamp(), listing.expires_at) {
        return Err(Error::ListingExpired);
    }
    if req.buyer == listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }
    if req.payment_asset != listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }
    if !payment_currency::is_supported(env, &req.payment_asset) {
        return Err(Error::UnsupportedAsset {
            asset: req.payment_asset.clone(),
        });
    }
    if req.max_price < listing.price {
        return Err(Error::InsufficientPayment {
            expected: listing.price,
            provided: req.max_price,
        });
    }

    let buyer_balance = token::Client::new(env, &req.payment_asset).balance(&req.buyer);
    if buyer_balance < listing.price {
        return Err(Error::InsufficientBalance);
    }
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;
    Ok(())
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
            Err(Error::UnsupportedAsset {
                asset: asset2.clone()
            })
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
            Err(Error::UnsupportedAsset {
                asset: unsupported_asset.clone()
            })
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
            Err(Error::InsufficientPayment {
                expected: 1_000,
                provided: 999
            })
            Err(Error::InvalidSalePrice)
        );
    }
    #[ignore]
    #[test]
    fn insufficient_payment_reports_expected_and_provided() {
        let env = Env::default();
        let seller = Address::generate(&env);
    fn rejected_when_seller_not_current_owner() {
        let env = Env::default();
        let original_seller = Address::generate(&env);
        let other_owner = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 5_000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1),
            Err(Error::InsufficientPayment {
                expected: 5_000,
                provided: 1
            })
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
            Err(Error::UnauthorizedOwnerError)
            Err(Error::Unauthorized)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_seller_not_current_owner_via_stored_listing() {
        let env = Env::default();
        let original_seller = Address::generate(&env);
        let new_owner = Address::generate(&env);
    fn validate_purchase_request_rejects_insufficient_payment() {
    fn rejected_when_token_is_frozen() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        // Listing is created while `original_seller` owns the token, then the
        // token is transferred away so the listing becomes stale.
        let listing = setup_token_and_listing(
            &env,
            1,
            &original_seller,
            &asset,
            1_000,
            0,
            ListingStatus::Active,
        );
        token_owner_storage::assign_owner(&env, 1, &new_owner, 1).unwrap();

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::UnauthorizedOwnerError)
        );
        assert_eq!(
            validate_purchase_for_token(&env, 1, &buyer, &asset, 1_000),
            Err(Error::UnauthorizedOwnerError)
        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        let req = PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: asset.clone(),
            max_price: 500,
        };

        assert_eq!(
            validate_purchase_request(&env, &req, &listing),
            Err(Error::InsufficientPayment {
                expected: 1_000,
                provided: 500
            })
        crate::frozen_token::freeze_token(&env, 1);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        );
    }
    #[ignore]
    #[test]
    fn validate_purchase_request_rejects_unsupported_asset() {
    fn validate_purchase_request_works() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        let unsupported_asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);

        let req = PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: unsupported_asset.clone(),
            max_price: 1_000,
        };

        assert_eq!(
            validate_purchase_request(&env, &req, &listing),
            Err(Error::UnsupportedAsset {
                asset: unsupported_asset.clone()
            })
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
            Err(Error::ListingNotFound)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_seller_not_current_owner() {
        let env = Env::default();
        let original_seller = Address::generate(&env);
        let other_owner = Address::generate(&env);
    fn validate_purchase_for_token_returns_listing_not_found() {
        let env = Env::default();
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
        // No listing has been saved for token 42.
        assert_eq!(
            validate_purchase_for_token(&env, 42, &buyer, &asset, 1_000),
            Err(Error::ListingNotFound)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_token_is_frozen() {
        let env = Env::default();
        let seller = Address::generate(&env);
    fn validate_purchase_request_returns_listing_not_found() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);
        crate::frozen_token::freeze_token(&env, 1);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::Unauthorized)
        let req = PurchaseRequest {
            listing_id: 7,
            buyer: buyer.clone(),
            payment_asset: asset.clone(),
            max_price: 1_000,
        };

        assert_eq!(
            validate_purchase_request(&env, &req, &Listing {
                token_id: 7,
                seller: Address::generate(&env),
                price: 1_000,
                payment_asset: asset.clone(),
                expires_at: 0,
                status: ListingStatus::Active,
                created_at: 0,
                buyer: None,
                sold_at: None,
            }),
            Ok(())
        );
    }
    #[ignore]
    #[test]
    fn validate_purchase_request_works() {
    fn rejected_when_listing_expired_via_request() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Active);
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 1, ListingStatus::Active);

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
        assert_eq!(
            validate_purchase_request(&env, &req, &listing),
            Err(Error::ListingExpired)
        );
    }
    #[ignore]
    #[test]
    fn rejected_when_listing_status_expired() {
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
        let listing =
            setup_token_and_listing(&env, 1, &seller, &asset, 1_000, 0, ListingStatus::Expired);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1_000),
            Err(Error::ListingNotActive)
        );
    }
}
