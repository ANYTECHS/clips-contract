//! Listing storage — persistence layer for marketplace listings.
//!
//! # Storage
//! Key: `DataKey::Listing(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

use super::types::{Listing, ListingStatus};

/// Save a listing. Overwrites any existing listing for the same token.
pub fn save_listing(env: &Env, listing: &Listing) {
    env.storage()
        .persistent()
        .set(&DataKey::Listing(listing.token_id), listing);
}

/// Load a listing by token ID. Returns `Err(TokenNotFound)` if absent.
pub fn get_listing(env: &Env, token_id: TokenId) -> Result<Listing, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Listing(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Check whether a listing exists for the given token.
pub fn has_listing(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Listing(token_id))
}

/// Update the status of an existing listing.
pub fn update_listing_status(
    env: &Env,
    token_id: TokenId,
    status: ListingStatus,
) -> Result<(), Error> {
    let mut listing = get_listing(env, token_id)?;
    if listing.status == ListingStatus::Sold {
        return Err(Error::InvalidConfig);
    }
    listing.status = status;
    save_listing(env, &listing);
    Ok(())
}

/// Mark a listing as sold, recording the buyer and sale timestamp (#883).
///
/// Returns `Err(ListingAlreadySold)` if the listing is already in Sold status.
pub fn mark_as_sold(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
) -> Result<(), Error> {
    let mut listing = get_listing(env, token_id)?;
    if listing.status == ListingStatus::Sold {
        return Err(Error::ListingAlreadySold);
    }
    listing.status = ListingStatus::Sold;
    listing.buyer = Some(buyer.clone());
    listing.sold_at = Some(env.ledger().timestamp());
    save_listing(env, &listing);
    Ok(())
}

/// Remove a listing from storage.
pub fn remove_listing(env: &Env, token_id: TokenId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Listing(token_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn listing(env: &Env, status: ListingStatus) -> Listing {
        Listing {
            token_id: 1,
            seller: Address::generate(env),
            price: 100,
            payment_asset: Address::generate(env),
            expires_at: 0,
            status,
            created_at: 0,
        }
    }

    #[test]
    fn sold_listing_cannot_be_updated() {
        let env = Env::default();
        save_listing(&env, &listing(&env, ListingStatus::Sold));

        assert_eq!(
            update_listing_status(&env, 1, ListingStatus::Active),
            Err(Error::InvalidConfig)
        );
        assert_eq!(get_listing(&env, 1).unwrap().status, ListingStatus::Sold);
    }

    #[test]
    fn non_sold_listing_can_be_updated() {
        let env = Env::default();
        save_listing(&env, &listing(&env, ListingStatus::Active));

        assert!(update_listing_status(&env, 1, ListingStatus::Cancelled).is_ok());
        assert_eq!(
            get_listing(&env, 1).unwrap().status,
            ListingStatus::Cancelled
        );
    }
}
