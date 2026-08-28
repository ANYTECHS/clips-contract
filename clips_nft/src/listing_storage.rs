use soroban_sdk::Env;

use crate::{listing_id_generator, types::{DataKey, Error, ListingId, TokenId}, ListingRequest};

/// Store a listing only when the token has no active listing.
pub fn create_listing(env: &Env, listing: &mut ListingRequest) -> Result<ListingId, Error> {
    let key = DataKey::ActiveListing(listing.token_id);
    if env.storage().persistent().has(&key) {
        return Err(Error::DuplicateListing);
    }
    let listing_id = listing_id_generator::generate_listing_id(env)?;
    listing.listing_id = listing_id;
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
        }
    }

    #[test]
    fn rejects_duplicate_active_listing() {
        let env = Env::default();
        let first = listing(&env, 1);

        assert_eq!(create_listing(&env, &first).unwrap(), 1);
        assert_eq!(create_listing(&env, &listing(&env, 1)), Err(Error::DuplicateListing));
    }

    #[test]
    fn allows_relisting_after_removal() {
        let env = Env::default();
        let first = listing(&env, 1);

        assert_eq!(create_listing(&env, &first).unwrap(), 1);
        remove_listing(&env, 1).unwrap();
        assert_eq!(create_listing(&env, &listing(&env, 1)).unwrap(), 2);
    }
}