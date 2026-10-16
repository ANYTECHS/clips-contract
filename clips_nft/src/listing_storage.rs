use soroban_sdk::Env;

use crate::{
    listing_id_generator,
    types::{DataKey, Error, ListingId, ListingStatus, TokenId},
    ListingRequest,
};

/// Store a listing only when the token has no active listing.
pub fn create_listing(env: &Env, listing: &mut ListingRequest) -> Result<ListingId, Error> {
    let key = DataKey::ActiveListing(listing.token_id);
    if env.storage().persistent().has(&key) {
        return Err(Error::DuplicateListing);
    }
    let listing_id = listing_id_generator::generate_listing_id(env)?;
    listing.listing_id = listing_id;
    listing.status = ListingStatus::Active;
    env.storage().persistent().set(&key, listing);
    crate::nft_listed_event::emit_nft_listed(
        env,
        listing_id,
        listing.token_id,
        &listing.seller,
        listing.price,
        &listing.payment_asset,
        env.ledger().timestamp(),
    );
    Ok(listing_id)
}

pub fn get_listing(env: &Env, token_id: TokenId) -> Result<ListingRequest, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::ActiveListing(token_id))
        .ok_or(Error::ListingNotFound)
}

/// Remove a listing when it is cancelled or completed by a sale.
pub fn remove_listing(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let key = DataKey::ActiveListing(token_id);
    if !env.storage().persistent().has(&key) {
        return Err(Error::ListingNotFound);
    }
    env.storage().persistent().remove(&key);
    Ok(())
}

/// Persist an updated listing in place.
///
/// Errors with [`Error::ListingNotFound`] when there is no active listing for the token.
pub fn save_listing(env: &Env, listing: &ListingRequest) -> Result<(), Error> {
    let key = DataKey::ActiveListing(listing.token_id);
    if !env.storage().persistent().has(&key) {
        return Err(Error::ListingNotFound);
    }
    env.storage().persistent().set(&key, listing);
    Ok(())
}

/// Update only the `status` field of the listing for `token_id`.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`.
pub fn update_listing_status(
    env: &Env,
    token_id: TokenId,
    new_status: ListingStatus,
) -> Result<(), Error> {
    let mut listing = get_listing(env, token_id)?;
    listing.status = new_status;
    env.storage()
        .persistent()
        .set(&DataKey::ActiveListing(token_id), &listing);
    Ok(())
}

/// Allow a seller to update price and/or expiration of an active listing.
///
/// Validates modification authority (seller & active state), validates the
/// new price and expiration values, persists the updated listing, and emits a
/// `ListingUpdatedEvent` for off-chain indexers.
///
/// # Errors
/// - [`Error::ListingNotFound`] if no listing exists for `token_id`.
/// - [`Error::Unauthorized`] if `seller` is not authorized to modify the listing.
/// - [`Error::InvalidSalePrice`] if `new_price` is non-positive.
/// - [`Error::PriceOverflow`] if `new_price` exceeds allowed maximum.
/// - [`Error::InvalidConfig`] if `new_expiration` is in the past.
pub fn update_listing(
    env: &Env,
    seller: &soroban_sdk::Address,
    token_id: TokenId,
    new_price: i128,
    new_expiration: u64,
) -> Result<(), Error> {
    // 1. Authorization & state checks (seller must auth and listing must be active).
    crate::listing_modification_guard::require_modification_auth(env, seller, token_id)?;

    // 2. Retrieve current listing.
    let mut listing = get_listing(env, token_id)?;

    // 3. Validate new price.
    if new_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    // Mirror the maximum allowed price used elsewhere in the contract.
    const MAX_LISTING_PRICE: i128 = i128::MAX / 2;
    if new_price > MAX_LISTING_PRICE {
        return Err(Error::PriceOverflow);
    }

    // 4. Validate new expiration (0 = never expires).
    let now = env.ledger().timestamp();
    if new_expiration != 0 && new_expiration < now {
        return Err(Error::InvalidConfig);
    }

    // 5. Capture previous values for the event.
    let old_price = listing.price;
    let old_expires_at = listing.expiration;

    // 6. Persist changes.
    listing.price = new_price;
    listing.expiration = new_expiration;
    save_listing(env, &listing)?;

    // 7. Emit update event using the listing's assigned id and payment asset.
    crate::events::listing::emit_listing_updated(
        env,
        listing.listing_id,
        listing.token_id,
        &listing.seller,
        &listing.payment_asset,
        old_price,
        new_price,
        old_expires_at,
        new_expiration,
        now,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn listing(env: &Env, token_id: TokenId) -> ListingRequest {
        ListingRequest {
            listing_id: 0,
            token_id,
            price: 100,
            payment_asset: Address::generate(env),
            expiration: 1_700_000_000,
            seller: Address::generate(env),
            status: ListingStatus::Active,
        }
    }
    #[ignore]
    #[test]
    fn rejects_duplicate_active_listing() {
        let env = Env::default();
        let mut first = listing(&env, 1);

        assert_eq!(create_listing(&env, &mut first).unwrap(), 1);
        let mut second = listing(&env, 1);
        assert_eq!(
            create_listing(&env, &mut second),
            Err(Error::DuplicateListing)
        );
    }
    #[ignore]
    #[test]
    fn allows_relisting_after_removal() {
        let env = Env::default();
        let mut first = listing(&env, 1);

        assert_eq!(create_listing(&env, &mut first).unwrap(), 1);
        remove_listing(&env, 1).unwrap();
        let mut second = listing(&env, 1);
        assert_eq!(create_listing(&env, &mut second).unwrap(), 2);
    }

    #[test]
    fn update_listing_status_changes_to_cancelled() {
        let env = Env::default();
        let mut lst = listing(&env, 1);
        create_listing(&env, &mut lst).unwrap();
        update_listing_status(&env, 1, ListingStatus::Cancelled).unwrap();
        let stored = get_listing(&env, 1).unwrap();
        assert_eq!(stored.status, ListingStatus::Cancelled);
    }

    #[test]
    fn update_listing_status_missing_returns_error() {
        let env = Env::default();
        assert_eq!(
            update_listing_status(&env, 99, ListingStatus::Cancelled),
            Err(Error::ListingNotFound)
        );
    }

    #[test]
    fn update_listing_allows_seller_to_change_price_and_expiration_and_emits_event() {
        use soroban_sdk::{symbol_short, testutils::Address as _, xdr::ContractEventBody, Address, Env, Symbol, TryFromVal};
        let env = Env::default();
        let mut lst = listing(&env, 1);
        let listing_id = create_listing(&env, &mut lst).unwrap();
        let seller = lst.seller.clone();

        // Update price and expiration
        update_listing(&env, &seller, 1, 200, 2_000).unwrap();

        // Verify storage updated
        let stored = get_listing(&env, 1).unwrap();
        assert_eq!(stored.price, 200);
        assert_eq!(stored.expiration, 2_000);
        assert_eq!(stored.listing_id, listing_id);

        // Verify event was emitted
        let expected_topic: Symbol = symbol_short!("lst_upd");
        let mut found = false;
        for event in env.events().all().events() {
            if let ContractEventBody::V0(v0) = &event.body {
                if v0.topics.len() == 1 && Symbol::try_from_val(&env, &v0.topics[0]) == Ok(expected_topic.clone()) {
                    let evt = crate::events::listing::ListingUpdatedEvent::try_from_val(&env, &v0.data).ok();
                    if let Some(e) = evt {
                        assert_eq!(e.listing_id, listing_id);
                        assert_eq!(e.token_id, 1);
                        assert_eq!(e.old_price, 100);
                        assert_eq!(e.new_price, 200);
                        assert_eq!(e.old_expires_at, 1_700_000_000);
                        assert_eq!(e.new_expires_at, 2_000);
                        found = true;
                    }
                }
            }
        }
        assert!(found, "Expected ListingUpdatedEvent to be emitted");
    }

    #[test]
    fn update_listing_rejects_non_seller() {
        let env = Env::default();
        let mut lst = listing(&env, 2);
        create_listing(&env, &mut lst).unwrap();
        let non_seller = Address::generate(&env);
        assert_eq!(update_listing(&env, &non_seller, 2, 200, 0), Err(Error::Unauthorized));
    }

    #[test]
    fn update_listing_rejects_when_not_active() {
        let env = Env::default();
        let mut lst = listing(&env, 3);
        create_listing(&env, &mut lst).unwrap();
        update_listing_status(&env, 3, ListingStatus::Cancelled).unwrap();
        let seller = lst.seller.clone();
        assert_eq!(update_listing(&env, &seller, 3, 200, 0), Err(Error::Unauthorized));
    }

    #[test]
    fn update_listing_rejects_invalid_price_and_expiration() {
        let env = Env::default();
        let mut lst = listing(&env, 4);
        create_listing(&env, &mut lst).unwrap();
        let seller = lst.seller.clone();

        // invalid price
        assert_eq!(update_listing(&env, &seller, 4, 0, 0), Err(Error::InvalidSalePrice));

        // expiration in the past
        let past = env.ledger().timestamp().saturating_sub(10);
        assert_eq!(update_listing(&env, &seller, 4, 200, past), Err(Error::InvalidConfig));
    }
}
