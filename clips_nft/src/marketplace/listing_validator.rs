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
/// | `InvalidConfig` | Payment asset is not supported. |
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

    // 2. Price must be positive.
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }

    // 3. Payment asset must be supported (non-zero address).
    // An empty/zero address indicates an invalid asset contract.
    if payment_asset == &Address::default() {
        return Err(Error::InvalidConfig);
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
}
