#[cfg(test)]
mod tests {
    use crate::marketplace::listing_storage;
    use crate::marketplace::listing_validator::validate_listing;
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::pause_state::save_pause_state;
    use crate::token_owner_storage;
    use crate::types::Error;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_token(env: &Env, token_id: u32, owner: &Address) {
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
    fn rejected_when_invalid_payment_asset() {
        let env = Env::default();
        let seller = Address::generate(&env);
        setup_token(&env, 1, &seller);

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &Address::default(), 0),
            Err(Error::InvalidConfig)
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
            },
        );

        assert_eq!(
            validate_listing(&env, &seller, 1, 1000, &asset, 0),
            Err(Error::DuplicateRecord)
        );
    }

    #[test]
    fn allows_new_listing_after_previous_sold() {
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
                status: ListingStatus::Sold,
                created_at: 0,
            },
        );

        assert!(validate_listing(&env, &seller, 1, 1000, &asset, 0).is_ok());
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
