//! Listing validator — reusable pre-condition checks for marketplace listings.
//!
//! Validates all invariants before a listing is created or modified:
//!
//! 1. The NFT must exist.
//! 2. The caller must be the token's owner.
//! 3. The price must be positive.
//! 4. The payment asset must be a supported currency.
//! 5. There must not already be an active listing for this token.
//! 6. If an expiration is set, it must be in the future.
//!
//! # Usage
//!
//! ```rust,ignore
//! listing_validator::validate_listing(env, &seller, token_id, price, &payment_asset, expires_at)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::pause_guard;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

use super::listing_storage;

/// Maximum allowed listing price in stroops. Prevents overflow in downstream
/// royalty and fee calculations that multiply the sale price by basis-point
/// factors. Set to half of `i128::MAX` to leave headroom for multiplication.
const MAX_LISTING_PRICE: i128 = i128::MAX / 2;

/// Validate all pre-conditions for creating a new listing.
///
/// # Arguments
/// * `env`          — Contract environment.
/// * `seller`       — Address of the prospective seller (must auth).
/// * `token_id`     — NFT to list.
/// * `price`        — Asking price in stroops.
/// * `payment_asset`— Soroban address of the accepted payment token.
/// * `expires_at`   — Unix timestamp when the listing expires (`0` = never).
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ContractPaused` | Contract is paused. |
/// | `TokenNotFound` | NFT does not exist. |
/// | `Unauthorized` | Caller is not the token owner. |
/// | `InvalidSalePrice` | Price is zero or negative. |
/// | `PriceOverflow` | Price exceeds the maximum allowed value. |
/// | `UnsupportedAsset` | Payment asset is not in the supported currencies list. |
/// | `DuplicateRecord` | An active listing already exists for this token. |
/// | `InvalidConfig` | Expiration is in the past. |
pub fn validate_listing(
    env: &Env,
    seller: &Address,
    token_id: TokenId,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
) -> Result<(), Error> {
    // 0. Contract must not be paused.
    pause_guard::require_not_paused(env)?;

    // 1. NFT must exist and caller must be the owner.
    let owner = token_owner_storage::get_owner(env, token_id)?;
    if *seller != owner {
        return Err(Error::Unauthorized);
    }

    // 2. Price must be positive and within bounds.
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    if price > MAX_LISTING_PRICE {
        return Err(Error::PriceOverflow);
    }

    // 3. Payment asset must be a supported currency.
    if !crate::payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }

    // 4. No active listing must already exist for this token.
    if let Ok(existing) = listing_storage::get_listing(env, token_id) {
        if existing.status == super::types::ListingStatus::Active {
            return Err(Error::DuplicateRecord);
        }
    }

    // 5. Expiration must be in the future (if set).
    if expires_at > 0 {
        let now = env.ledger().timestamp();
        if expires_at <= now {
            return Err(Error::InvalidConfig);
        }
    }

    Ok(())
}

/// Cancel an active marketplace listing.
///
/// The caller must be the listing's seller, an operator approved by the
/// seller, or the contract admin. The listing must be in `Active` status.
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `caller`   — Address requesting cancellation (must auth).
/// * `token_id` — Token whose listing should be cancelled.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `TokenNotFound` | No listing exists for this token. |
/// | `ListingNotActive` | Listing is already Sold or Cancelled. |
/// | `Unauthorized` | Caller is not the seller, an approved operator, or the admin. |
pub fn cancel_listing(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    caller.require_auth();

    let listing = listing_storage::get_listing(env, token_id)?;

    if listing.status != super::types::ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }

    // Authorization: seller, operator approved by seller, or contract admin.
    let is_seller = *caller == listing.seller;
    let is_operator = crate::operator_approval::is_operator(env, &listing.seller, caller);
    let is_admin = env
        .storage()
        .instance()
        .get::<_, Address>(&crate::types::DataKey::Admin)
        .map_or(false, |admin| *caller == admin);

    if !is_seller && !is_operator && !is_admin {
        return Err(Error::Unauthorized);
    }

    // Update status to Cancelled.
    listing_storage::update_listing_status(env, token_id, super::types::ListingStatus::Cancelled)?;

    // Emit cancellation event via the centralized event module.
    crate::events::listing::emit_listing_cancelled(
        env,
        token_id,
        &listing.seller,
        caller,
        env.ledger().timestamp(),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::listing_storage;
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::pause_state::save_pause_state;
    use crate::token_owner_storage;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    #[test]
    fn valid_listing_passes() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert!(validate_listing(&env, &seller, 1, 1000, &asset, 0).is_ok());
    }

    #[test]
    fn rejected_when_paused() {
        let env = Env::default();
        save_pause_state(&env, true);
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &asset, 0),
            Err(Error::ContractPaused)
        );
    }

    #[test]
    fn rejected_when_not_owner() {
        let env = Env::default();
        let owner = Address::generate(&env);
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &owner);

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &asset, 0),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn rejected_when_price_zero() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, 0, &asset, 0),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn rejected_when_price_negative() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, -100, &asset, 0),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn rejected_when_duplicate_active_listing() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        listing_storage::save_listing(
            &env,
            &Listing {
                token_id: 1,
                seller: seller.clone(),
                price: 500,
                payment_asset: asset.clone(),
                expires_at: 0,
                status: ListingStatus::Active,
                created_at: 0,
                buyer: None,
                sold_at: None,
            },
        );

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &asset, 0),
            Err(Error::DuplicateRecord)
        );
    }

    #[test]
    fn rejected_when_expiration_in_past() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &asset, 1),
            Err(Error::InvalidConfig)
        );
    }

    // ── Price overflow tests (#865) ──────────────────────────────────────────

    #[test]
    fn rejected_when_price_exceeds_max() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, i128::MAX, &asset, 0),
            Err(Error::PriceOverflow)
        );
    }

    #[test]
    fn accepts_price_at_upper_bound() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert!(validate_listing(&env, &seller, 1, MAX_LISTING_PRICE, &asset, 0).is_ok());
    }

    // ── Supported payment asset tests (#870) ─────────────────────────────────

    #[test]
    fn rejected_when_unsupported_payment_asset() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let unsupported_asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &unsupported_asset, 0),
            Err(Error::UnsupportedAsset)
        );
    }

    #[test]
    fn accepted_when_payment_asset_is_supported() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let supported_asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        crate::payment_currency::add_currency(&env, supported_asset.clone()).unwrap();

        assert!(validate_listing(&env, &seller, 1, 1000, &supported_asset, 0).is_ok());
    }

    // ── Cancel listing tests (#866, #869) ────────────────────────────────────

    fn setup_active_listing(env: &Env, token_id: TokenId, seller: &Address, asset: &Address) {
        listing_storage::save_listing(
            env,
            &Listing {
                token_id,
                seller: seller.clone(),
                price: 1000,
                payment_asset: asset.clone(),
                expires_at: 0,
                status: ListingStatus::Active,
                created_at: 0,
                buyer: None,
                sold_at: None,
            },
        );
    }

    #[test]
    fn cancel_listing_succeeds_for_seller() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);
        setup_active_listing(&env, 1, &seller, &asset);

        assert!(cancel_listing(&env, &seller, 1).is_ok());

        let listing = listing_storage::get_listing(&env, 1).unwrap();
        assert_eq!(listing.status, ListingStatus::Cancelled);
    }

    #[test]
    fn cancel_listing_fails_when_not_active() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);

        listing_storage::save_listing(
            &env,
            &Listing {
                token_id: 1,
                seller: seller.clone(),
                price: 1000,
                payment_asset: asset.clone(),
                expires_at: 0,
                status: ListingStatus::Sold,
                created_at: 0,
                buyer: None,
                sold_at: None,
            },
        );

        assert_eq!(cancel_listing(&env, &seller, 1), Err(Error::ListingNotActive));
    }

    #[test]
    fn cancel_listing_fails_when_unauthorized() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let unauthorized = Address::generate(&env);
        setup_token(&env, 1, &seller);
        setup_active_listing(&env, 1, &seller, &asset);

        assert_eq!(
            cancel_listing(&env, &unauthorized, 1),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn cancel_listing_succeeds_for_operator() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let operator = Address::generate(&env);
        let asset = Address::generate(&env);
        setup_token(&env, 1, &seller);
        setup_active_listing(&env, 1, &seller, &asset);

        crate::operator_approval::save_operator(&env, &seller, &operator);

        assert!(cancel_listing(&env, &operator, 1).is_ok());

        let listing = listing_storage::get_listing(&env, 1).unwrap();
        assert_eq!(listing.status, ListingStatus::Cancelled);
    }

    #[test]
    fn cancel_listing_fails_when_no_listing_exists() {
        let env = Env::default();
        let seller = Address::generate(&env);

        assert_eq!(cancel_listing(&env, &seller, 999), Err(Error::TokenNotFound));
    }
}
