use soroban_sdk::Env;

use crate::{
    listing_id_generator,
    types::{DataKey, Error, ListingId, ListingStatus, TokenId},
    ListingRequest,
};

/// Store a listing only when the token has no active listing.
pub fn create_listing(env: &Env, listing: &mut ListingRequest) -> Result<ListingId, Error> {
    let key = DataKey::ActiveListing(listing.token_id);
    // Only reject if there is an *active* listing; sold/cancelled/expired listings
    // should allow relisting.
    if env.storage().persistent().has(&key) {
        let existing: ListingRequest = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ListingNotFound)?;
        if existing.status == ListingStatus::Active {
            return Err(Error::DuplicateListing);
        }
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
}
