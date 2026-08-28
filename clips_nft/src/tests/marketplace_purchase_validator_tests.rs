#[cfg(test)]
mod tests {
    use crate::marketplace::listing_storage;
    use crate::marketplace::purchase_validator::{
        validate_purchase, validate_purchase_for_token, validate_purchase_request,
    };
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::pause_state::save_pause_state;
    use crate::purchase_request::PurchaseRequest;
    use crate::token_owner_storage;
    use crate::types::Error;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_token(env: &Env, token_id: u32, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    fn setup_listing(
        env: &Env,
        token_id: u32,
        seller: &Address,
        asset: &Address,
        price: i128,
        expires_at: u64,
        status: ListingStatus,
    ) -> Listing {
        setup_token(env, token_id, seller);
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
    fn valid_purchase_validation_succeeds() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        assert!(validate_purchase(&env, &buyer, &listing, &asset, 1000).is_ok());
    }

    #[test]
    fn valid_purchase_request_succeeds() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        let req = PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: asset.clone(),
            max_price: 1500,
        };

        assert!(validate_purchase_request(&env, &req, &listing).is_ok());
    }

    #[test]
    fn rejected_when_contract_is_paused() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);
        save_pause_state(&env, true);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1000),
            Err(Error::ContractPaused)
        );
    }

    #[test]
    fn rejected_when_listing_inactive_or_sold() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Sold);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1000),
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

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 1, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1000),
            Err(Error::OfferExpired)
        );
    }

    #[test]
    fn rejected_when_buyer_is_seller() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &seller, &listing, &asset, 1000),
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

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1000),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn rejected_when_unsupported_payment_asset() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1000),
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

        let listing = setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 999),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn rejected_when_seller_not_nft_owner() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let actual_owner = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        setup_token(&env, 1, &actual_owner);
        let listing = Listing {
            token_id: 1,
            seller: seller.clone(),
            price: 1000,
            payment_asset: asset.clone(),
            expires_at: 0,
            status: ListingStatus::Active,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };
        listing_storage::save_listing(&env, &listing);

        assert_eq!(
            validate_purchase(&env, &buyer, &listing, &asset, 1000),
            Err(Error::Unauthorized)
        );
    }

    #[test]
    fn validate_purchase_for_token_loads_listing() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();

        setup_listing(&env, 1, &seller, &asset, 1000, 0, ListingStatus::Active);

        let res = validate_purchase_for_token(&env, 1, &buyer, &asset, 1000);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().token_id, 1);
    }
}
