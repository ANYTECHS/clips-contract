//! Tests for listing expiration validation (issue #876) and
//! `ListingStatus` enum lifecycle (issue #877).
//!
//! ## Listing Expiration Validation
//! - Ledger timestamp comparison (before / at / after expiry).
//! - Rejection of expired listings returns `Error::ListingExpired`.
//! - `expires_at == 0` is treated as "no expiration".
//!
//! ## ListingStatus Enum
//! - All four variants are distinct (`Active`, `Sold`, `Cancelled`, `Expired`).
//! - Serialization round-trip via `contracttype` derive.
//! - Status transitions via storage helpers.

#[cfg(test)]
mod listing_expiration_tests {
    use crate::marketplace::listing_storage;
    use crate::marketplace::purchase_state_guard::{
        check_listing_not_expired, get_purchasable_listing, require_purchasable,
        require_purchasable_listing,
    };
    use crate::marketplace::purchase_validator::validate_purchase;
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::token_owner_storage;
    use crate::types::Error;
    use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_listing(env: &Env, token_id: u32, expires_at: u64, status: ListingStatus) -> Listing {
        let seller = Address::generate(env);
        token_owner_storage::assign_owner(env, token_id, &seller, token_id).unwrap();
        let listing = Listing {
            token_id,
            seller,
            price: 1_000,
            payment_asset: Address::generate(env),
            expires_at,
            status,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };
        listing_storage::save_listing(env, &listing);
        listing
    }

    // ── Ledger timestamp comparison tests ────────────────────────────────────

    /// Listing with `expires_at == 0` never expires regardless of ledger time.
    #[test]
    fn no_expiration_zero_passes_at_any_timestamp() {
        let env = Env::default();
        env.ledger().set_timestamp(999_999_999);
        let listing = make_listing(&env, 1, 0, ListingStatus::Active);

        assert!(
            check_listing_not_expired(&env, &listing).is_ok(),
            "listing with expires_at == 0 must never expire"
        );
    }

    /// Listing with a future expiration is purchasable.
    #[test]
    fn listing_with_future_expiration_is_valid() {
        let env = Env::default();
        env.ledger().set_timestamp(1_000);
        let listing = make_listing(&env, 2, 2_000, ListingStatus::Active);

        assert!(
            check_listing_not_expired(&env, &listing).is_ok(),
            "listing expiring in the future must not be rejected"
        );
    }

    /// One second before expiry the listing is still valid.
    #[test]
    fn one_second_before_expiry_is_still_valid() {
        let env = Env::default();
        env.ledger().set_timestamp(1_999);
        let listing = make_listing(&env, 3, 2_000, ListingStatus::Active);

        assert!(
            check_listing_not_expired(&env, &listing).is_ok(),
            "listing must still be valid 1 second before expiry"
        );
    }

    /// At the exact expiration timestamp the listing is expired.
    #[test]
    fn exact_expiry_timestamp_is_expired() {
        let env = Env::default();
        env.ledger().set_timestamp(2_000);
        let listing = make_listing(&env, 4, 2_000, ListingStatus::Active);

        assert_eq!(
            check_listing_not_expired(&env, &listing),
            Err(Error::ListingExpired),
            "listing must be expired when ledger time == expires_at"
        );
    }

    /// After the expiration timestamp the listing is expired.
    #[test]
    fn past_expiry_timestamp_is_expired() {
        let env = Env::default();
        env.ledger().set_timestamp(5_000);
        let listing = make_listing(&env, 5, 1_000, ListingStatus::Active);

        assert_eq!(
            check_listing_not_expired(&env, &listing),
            Err(Error::ListingExpired),
            "listing must be expired when ledger time > expires_at"
        );
    }

    // ── Custom error tests ────────────────────────────────────────────────────

    /// Expiration rejection MUST return the dedicated `ListingExpired` error
    /// (not the generic `OfferExpired` or `InvalidConfig`).
    #[test]
    fn expired_listing_returns_listing_expired_not_offer_expired() {
        let env = Env::default();
        env.ledger().set_timestamp(3_000);
        let listing = make_listing(&env, 6, 1_000, ListingStatus::Active);

        let err = check_listing_not_expired(&env, &listing)
            .expect_err("expired listing must return an error");

        assert_eq!(
            err,
            Error::ListingExpired,
            "expiration must use Error::ListingExpired (not OfferExpired or other)"
        );
        // Explicitly confirm it is NOT the old OfferExpired variant.
        assert_ne!(
            err,
            Error::OfferExpired,
            "must not use the generic OfferExpired error for listing expiration"
        );
    }

    /// `require_purchasable` returns `ListingExpired` for an expired listing
    /// loaded from storage.
    #[test]
    fn require_purchasable_returns_listing_expired_for_expired_listing() {
        let env = Env::default();
        env.ledger().set_timestamp(4_000);
        make_listing(&env, 7, 500, ListingStatus::Active);

        assert_eq!(
            require_purchasable(&env, 7),
            Err(Error::ListingExpired),
            "require_purchasable must propagate ListingExpired"
        );
    }

    /// `get_purchasable_listing` returns `ListingExpired` for an expired listing.
    #[test]
    fn get_purchasable_listing_returns_listing_expired_for_expired_listing() {
        let env = Env::default();
        env.ledger().set_timestamp(4_000);
        make_listing(&env, 8, 500, ListingStatus::Active);

        assert_eq!(
            get_purchasable_listing(&env, 8),
            Err(Error::ListingExpired),
            "get_purchasable_listing must propagate ListingExpired"
        );
    }

    /// `validate_purchase` returns `ListingExpired` (not `OfferExpired`) when
    /// the listing's expiration timestamp has passed.
    #[test]
    fn validate_purchase_returns_listing_expired_for_expired_listing() {
        let env = Env::default();
        // expires_at = 1 triggers expiry at the default ledger timestamp (>= 1)
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let asset = Address::generate(&env);
        crate::payment_currency::add_currency(&env, asset.clone()).unwrap();
        token_owner_storage::assign_owner(&env, 10, &seller, 10).unwrap();

        let listing = Listing {
            token_id: 10,
            seller: seller.clone(),
            price: 1_000,
            payment_asset: asset.clone(),
            expires_at: 1,
            status: ListingStatus::Active,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };

        let err = validate_purchase(&env, &buyer, &listing, &asset, 1_000)
            .expect_err("expired listing must be rejected by validate_purchase");

        assert_eq!(
            err,
            Error::ListingExpired,
            "validate_purchase must return ListingExpired for expired listings"
        );
        assert_ne!(
            err,
            Error::OfferExpired,
            "validate_purchase must NOT use OfferExpired for listing expiration"
        );
    }

    /// `require_purchasable_listing` respects the ordering: sold > not_active > expired.
    #[test]
    fn sold_and_expired_listing_returns_listing_already_sold() {
        let env = Env::default();
        env.ledger().set_timestamp(5_000);
        let listing = make_listing(&env, 11, 1_000, ListingStatus::Sold);

        assert_eq!(
            require_purchasable_listing(&env, &listing),
            Err(Error::ListingAlreadySold),
            "sold status must take precedence over expired"
        );
    }
}

#[cfg(test)]
mod listing_status_enum_tests {
    use crate::marketplace::listing_storage;
    use crate::marketplace::types::{Listing, ListingStatus};
    use crate::token_owner_storage;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Variant existence tests ───────────────────────────────────────────────

    /// All four lifecycle variants exist and are distinct.
    #[test]
    fn all_four_variants_are_distinct() {
        assert_ne!(ListingStatus::Active, ListingStatus::Sold);
        assert_ne!(ListingStatus::Active, ListingStatus::Cancelled);
        assert_ne!(ListingStatus::Active, ListingStatus::Expired);
        assert_ne!(ListingStatus::Sold, ListingStatus::Cancelled);
        assert_ne!(ListingStatus::Sold, ListingStatus::Expired);
        assert_ne!(ListingStatus::Cancelled, ListingStatus::Expired);
    }

    /// Each variant equals itself (reflexivity).
    #[test]
    fn each_variant_equals_itself() {
        assert_eq!(ListingStatus::Active, ListingStatus::Active);
        assert_eq!(ListingStatus::Sold, ListingStatus::Sold);
        assert_eq!(ListingStatus::Cancelled, ListingStatus::Cancelled);
        assert_eq!(ListingStatus::Expired, ListingStatus::Expired);
    }

    // ── Serialization / storage round-trip tests ─────────────────────────────
    //
    // The `#[contracttype]` derive implements XDR serialization for Soroban
    // storage. We verify the round-trip by saving a listing with each status
    // and reading it back.

    fn make_listing_with_status(env: &Env, token_id: u32, status: ListingStatus) -> Listing {
        let seller = Address::generate(env);
        token_owner_storage::assign_owner(env, token_id, &seller, token_id).unwrap();
        let listing = Listing {
            token_id,
            seller,
            price: 500,
            payment_asset: Address::generate(env),
            expires_at: 0,
            status,
            created_at: 0,
            buyer: None,
            sold_at: None,
        };
        listing_storage::save_listing(env, &listing);
        listing
    }

    #[test]
    fn active_status_survives_storage_round_trip() {
        let env = Env::default();
        make_listing_with_status(&env, 1, ListingStatus::Active);
        let loaded = listing_storage::get_listing(&env, 1).unwrap();
        assert_eq!(loaded.status, ListingStatus::Active);
    }

    #[test]
    fn sold_status_survives_storage_round_trip() {
        let env = Env::default();
        make_listing_with_status(&env, 2, ListingStatus::Sold);
        let loaded = listing_storage::get_listing(&env, 2).unwrap();
        assert_eq!(loaded.status, ListingStatus::Sold);
    }

    #[test]
    fn cancelled_status_survives_storage_round_trip() {
        let env = Env::default();
        make_listing_with_status(&env, 3, ListingStatus::Cancelled);
        let loaded = listing_storage::get_listing(&env, 3).unwrap();
        assert_eq!(loaded.status, ListingStatus::Cancelled);
    }

    #[test]
    fn expired_status_survives_storage_round_trip() {
        let env = Env::default();
        make_listing_with_status(&env, 4, ListingStatus::Expired);
        let loaded = listing_storage::get_listing(&env, 4).unwrap();
        assert_eq!(
            loaded.status,
            ListingStatus::Expired,
            "Expired status must survive a storage round-trip"
        );
    }

    // ── Status update tests ───────────────────────────────────────────────────

    #[test]
    fn status_can_be_updated_from_active_to_sold() {
        let env = Env::default();
        make_listing_with_status(&env, 10, ListingStatus::Active);
        listing_storage::update_listing_status(&env, 10, ListingStatus::Sold).unwrap();
        let loaded = listing_storage::get_listing(&env, 10).unwrap();
        assert_eq!(loaded.status, ListingStatus::Sold);
    }

    #[test]
    fn status_can_be_updated_from_active_to_cancelled() {
        let env = Env::default();
        make_listing_with_status(&env, 11, ListingStatus::Active);
        listing_storage::update_listing_status(&env, 11, ListingStatus::Cancelled).unwrap();
        let loaded = listing_storage::get_listing(&env, 11).unwrap();
        assert_eq!(loaded.status, ListingStatus::Cancelled);
    }

    #[test]
    fn status_can_be_updated_from_active_to_expired() {
        let env = Env::default();
        make_listing_with_status(&env, 12, ListingStatus::Active);
        listing_storage::update_listing_status(&env, 12, ListingStatus::Expired).unwrap();
        let loaded = listing_storage::get_listing(&env, 12).unwrap();
        assert_eq!(
            loaded.status,
            ListingStatus::Expired,
            "Expired status must be settable via update_listing_status"
        );
    }

    // ── Clone trait tests ─────────────────────────────────────────────────────

    #[test]
    fn listing_status_is_cloneable() {
        let status = ListingStatus::Expired;
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }

    #[test]
    fn all_variants_are_cloneable() {
        let variants = [
            ListingStatus::Active,
            ListingStatus::Sold,
            ListingStatus::Cancelled,
            ListingStatus::Expired,
        ];
        for v in variants {
            let cloned = v.clone();
            assert_eq!(v, cloned);
        }
    }
}
