//! Purchase state guard — verifies that an NFT listing can still be purchased (issue #1027).
//!
//! Validates all purchase-state invariants before an NFT purchase is processed:
//!
//! 1. **Listing existence**: Verify listing exists for the given token ID
//!    ([`Error::ListingNotFound`]).
//! 2. **Listing not sold**: Reject listings that have already been purchased
//!    ([`Error::ListingAlreadySold`]).
//! 3. **Listing active**: Verify listing is in [`ListingStatus::Active`] status
//!    ([`Error::ListingNotActive`]).
//! 4. **Listing not expired**: Verify expiration timestamp has not arrived
//!    ([`Error::ListingExpired`]). Note that `expires_at == 0` signifies no expiration.
//!
//! # Usage
//!
//! Guarding a purchase with only a `token_id`:
//! ```rust,ignore
//! purchase_state_guard::require_purchasable(env, token_id)?;
//! ```
//!
//! Fetching the validated [`Listing`] in a single step:
//! ```rust,ignore
//! let listing = purchase_state_guard::get_purchasable_listing(env, token_id)?;
//! ```
//!
//! Validating an already-loaded [`Listing`]:
//! ```rust,ignore
//! purchase_state_guard::require_purchasable_listing(env, &listing)?;
//! ```

use soroban_sdk::Env;

use crate::types::{Error, TokenId};

use super::listing_storage;
use super::types::{Listing, ListingStatus};

// ─── Granular State Checks ───────────────────────────────────────────────────

/// Verify that a marketplace listing exists for the given `token_id`.
///
/// Returns the loaded [`Listing`] if found, or [`Error::ListingNotFound`] if
/// absent.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`.
pub fn check_listing_exists(env: &Env, token_id: TokenId) -> Result<Listing, Error> {
    listing_storage::get_listing(env, token_id).map_err(|_| Error::ListingNotFound)
}

/// Verify that a listing is not already marked as sold.
///
/// # Errors
/// Returns [`Error::ListingAlreadySold`] if `listing.status == ListingStatus::Sold`.
pub fn check_listing_not_sold(listing: &Listing) -> Result<(), Error> {
    if listing.status == ListingStatus::Sold {
        return Err(Error::ListingAlreadySold);
    }
    Ok(())
}

/// Verify that a listing is currently in [`ListingStatus::Active`] status.
///
/// # Errors
/// Returns [`Error::ListingNotActive`] if the listing status is not active.
pub fn check_listing_active(listing: &Listing) -> Result<(), Error> {
    if listing.status != ListingStatus::Active {
        return Err(Error::ListingNotActive);
    }
    Ok(())
}

/// Verify that a listing has not expired.
///
/// Listings with `expires_at == 0` do not expire.
/// If `expires_at > 0`, the listing is expired when `expires_at <= env.ledger().timestamp()`.
///
/// # Errors
/// Returns [`Error::ListingExpired`] if current ledger timestamp has reached or passed `expires_at`.
pub fn check_listing_not_expired(env: &Env, listing: &Listing) -> Result<(), Error> {
    if listing.expires_at > 0 {
        let now = env.ledger().timestamp();
        if listing.expires_at <= now {
            return Err(Error::ListingExpired);
        }
    }
    Ok(())
}

// ─── Composite Guards ────────────────────────────────────────────────────────

/// Verify all purchase state invariants on an already-loaded [`Listing`].
///
/// Checks:
/// 1. Reject already-sold listings ([`Error::ListingAlreadySold`]).
/// 2. Verify listing is active ([`Error::ListingNotActive`]).
/// 3. Verify listing has not expired ([`Error::ListingExpired`]).
///
/// # Errors
/// - [`Error::ListingAlreadySold`] if `listing.status == ListingStatus::Sold`.
/// - [`Error::ListingNotActive`] if `listing.status != ListingStatus::Active`.
/// - [`Error::ListingExpired`] if `expires_at > 0 && expires_at <= now`.
pub fn require_purchasable_listing(env: &Env, listing: &Listing) -> Result<(), Error> {
    check_listing_not_sold(listing)?;
    check_listing_active(listing)?;
    check_listing_not_expired(env, listing)?;
    Ok(())
}

/// Verify that an active, unexpired, unsold listing exists for `token_id`.
///
/// Checks:
/// 1. Verify listing exists in persistent storage ([`Error::ListingNotFound`]).
/// 2. Reject already-sold listings ([`Error::ListingAlreadySold`]).
/// 3. Verify listing is active ([`Error::ListingNotActive`]).
/// 4. Verify listing has not expired ([`Error::ListingExpired`]).
///
/// # Errors
/// - [`Error::ListingNotFound`] if no listing exists for `token_id`.
/// - [`Error::ListingAlreadySold`] if listing is in `Sold` status.
/// - [`Error::ListingNotActive`] if listing is not in `Active` status (e.g. `Cancelled`).
/// - [`Error::ListingExpired`] if listing expiration timestamp has passed.
pub fn require_purchasable(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let listing = check_listing_exists(env, token_id)?;
    require_purchasable_listing(env, &listing)
}

/// Verify purchase state and return the validated [`Listing`] in one call.
///
/// # Errors
/// See [`require_purchasable`].
pub fn get_purchasable_listing(env: &Env, token_id: TokenId) -> Result<Listing, Error> {
    let listing = check_listing_exists(env, token_id)?;
    require_purchasable_listing(env, &listing)?;
    Ok(listing)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_listing(
        env: &Env,
        token_id: TokenId,
        expires_at: u64,
        status: ListingStatus,
    ) -> Listing {
        Listing {
            token_id,
            seller: Address::generate(env),
            price: 1_000_000,
            payment_asset: Address::generate(env),
            expires_at,
            status,
            created_at: 0,
            buyer: None,
            sold_at: None,
        }
    }

    #[test]
    fn active_listing_no_expiration_passes() {
        let env = Env::default();
        let listing = make_listing(&env, 1, 0, ListingStatus::Active);
        listing_storage::save_listing(&env, &listing);

        assert!(require_purchasable(&env, 1).is_ok());
        assert!(require_purchasable_listing(&env, &listing).is_ok());
        assert_eq!(get_purchasable_listing(&env, 1).unwrap().token_id, 1);
    }

    #[test]
    fn active_listing_future_expiration_passes() {
        let env = Env::default();
        env.ledger().set_timestamp(1_000);
        let listing = make_listing(&env, 1, 2_000, ListingStatus::Active);
        listing_storage::save_listing(&env, &listing);

        assert!(require_purchasable(&env, 1).is_ok());
        assert!(require_purchasable_listing(&env, &listing).is_ok());
    }

    #[test]
    fn listing_not_found_returns_error() {
        let env = Env::default();

        assert_eq!(require_purchasable(&env, 999), Err(Error::ListingNotFound));
        assert_eq!(
            get_purchasable_listing(&env, 999),
            Err(Error::ListingNotFound)
        );
        assert_eq!(check_listing_exists(&env, 999), Err(Error::ListingNotFound));
    }

    #[test]
    fn sold_listing_returns_listing_already_sold() {
        let env = Env::default();
        let listing = make_listing(&env, 2, 0, ListingStatus::Sold);
        listing_storage::save_listing(&env, &listing);

        assert_eq!(require_purchasable(&env, 2), Err(Error::ListingAlreadySold));
        assert_eq!(
            require_purchasable_listing(&env, &listing),
            Err(Error::ListingAlreadySold)
        );
        assert_eq!(
            check_listing_not_sold(&listing),
            Err(Error::ListingAlreadySold)
        );
    }

    #[test]
    fn cancelled_listing_returns_listing_not_active() {
        let env = Env::default();
        let listing = make_listing(&env, 3, 0, ListingStatus::Cancelled);
        listing_storage::save_listing(&env, &listing);

        assert_eq!(require_purchasable(&env, 3), Err(Error::ListingNotActive));
        assert_eq!(
            require_purchasable_listing(&env, &listing),
            Err(Error::ListingNotActive)
        );
        assert_eq!(check_listing_active(&listing), Err(Error::ListingNotActive));
    }

    #[test]
    fn expired_listing_returns_listing_expired() {
        let env = Env::default();
        env.ledger().set_timestamp(2_000);
        let listing = make_listing(&env, 4, 1_000, ListingStatus::Active);
        listing_storage::save_listing(&env, &listing);

        assert_eq!(require_purchasable(&env, 4), Err(Error::ListingExpired));
        assert_eq!(
            require_purchasable_listing(&env, &listing),
            Err(Error::ListingExpired)
        );
        assert_eq!(
            check_listing_not_expired(&env, &listing),
            Err(Error::ListingExpired)
        );
    }

    #[test]
    fn exact_timestamp_expiry_is_rejected() {
        let env = Env::default();
        env.ledger().set_timestamp(1_500);
        let listing = make_listing(&env, 5, 1_500, ListingStatus::Active);
        listing_storage::save_listing(&env, &listing);

        assert_eq!(require_purchasable(&env, 5), Err(Error::ListingExpired));
    }

    #[test]
    fn one_second_before_expiry_is_allowed() {
        let env = Env::default();
        env.ledger().set_timestamp(1_499);
        let listing = make_listing(&env, 6, 1_500, ListingStatus::Active);
        listing_storage::save_listing(&env, &listing);

        assert!(require_purchasable(&env, 6).is_ok());
    }

    #[test]
    fn sold_takes_precedence_over_expired() {
        let env = Env::default();
        env.ledger().set_timestamp(5_000);
        // Both expired and sold
        let listing = make_listing(&env, 7, 1_000, ListingStatus::Sold);
        listing_storage::save_listing(&env, &listing);

        assert_eq!(require_purchasable(&env, 7), Err(Error::ListingAlreadySold));
    }

    #[test]
    fn removed_listing_returns_not_found() {
        let env = Env::default();
        let listing = make_listing(&env, 8, 0, ListingStatus::Active);
        listing_storage::save_listing(&env, &listing);
        assert!(require_purchasable(&env, 8).is_ok());

        listing_storage::remove_listing(&env, 8);
        assert_eq!(require_purchasable(&env, 8), Err(Error::ListingNotFound));
    }
}
