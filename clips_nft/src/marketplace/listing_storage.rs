//! Active-listing persistence, keyed by token.
//!
//! Mirrors the flat `listing_storage.rs` pattern but stores the canonical
//! [`Listing`](super::types::Listing) shape under `DataKey::ActiveListing`.
//! `save_listing` overwrites unconditionally: callers run
//! [`validate_listing`](super::listing_validator::validate_listing) first,
//! which rejects duplicates with `Error::DuplicateRecord`.

use soroban_sdk::Env;

use super::types::{Listing, ListingStatus};
use crate::types::{DataKey, Error, TokenId};

/// Persist (create or overwrite) the active listing for a token.
pub fn save_listing(env: &Env, listing: &Listing) {
    env.storage()
        .persistent()
        .set(&DataKey::ActiveListing(listing.token_id), listing);
}

/// Load the active listing for a token, if any.
pub fn get_listing(env: &Env, token_id: TokenId) -> Option<Listing> {
    env.storage()
        .persistent()
        .get(&DataKey::ActiveListing(token_id))
}

/// Remove the active listing (cancelled or sold).
pub fn remove_listing(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let key = DataKey::ActiveListing(token_id);
    if !env.storage().persistent().has(&key) {
        return Err(Error::ListingNotFound);
    }
    env.storage().persistent().remove(&key);
    Ok(())
}

/// True when an *active* listing exists for the token. Sold or cancelled
/// records do not block relisting.
pub fn has_active_listing(env: &Env, token_id: TokenId) -> bool {
    matches!(
        get_listing(env, token_id),
        Some(Listing {
            status: ListingStatus::Active,
            ..
        })
    )
//! Marketplace listing storage — persistent CRUD for [`Listing`] records.
//!
//! # Storage key
//! `DataKey::Listing(token_id)` — persistent storage, one entry per token.
//!
//! # Acceptance criteria (Issue #1271 — Create Listing Storage)
//! - **Store** a new listing ([`save_listing`]).
//! - **Retrieve** a listing by token ID ([`get_listing`], [`try_get_listing`]).
//! - **Update** an existing listing in-place ([`update_listing`],
//!   [`update_listing_status`]).
//! - **Delete** a listing ([`remove_listing`]).

use soroban_sdk::Env;

use crate::marketplace::types::{Listing, ListingStatus};
use crate::types::{DataKey, Error, TokenId};

// ── Write ─────────────────────────────────────────────────────────────────────

/// Persist `listing` under `DataKey::Listing(listing.token_id)`.
///
/// Overwrites any previously stored value — use this for both initial creation
/// and in-place field updates.
pub fn save_listing(env: &Env, listing: &Listing) {
    env.storage()
        .persistent()
        .set(&DataKey::Listing(listing.token_id), listing);
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Return the [`Listing`] for `token_id`.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`.
pub fn get_listing(env: &Env, token_id: TokenId) -> Result<Listing, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Listing(token_id))
        .ok_or(Error::ListingNotFound)
}

/// Return `Some(Listing)` if one exists for `token_id`, or `None` otherwise.
pub fn try_get_listing(env: &Env, token_id: TokenId) -> Option<Listing> {
    env.storage()
        .persistent()
        .get(&DataKey::Listing(token_id))
}

/// Return `true` if a listing record exists for `token_id`.
pub fn has_listing(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Listing(token_id))
}

// ── Update ────────────────────────────────────────────────────────────────────

/// Replace the stored listing with `listing`.
///
/// Requires that a listing already exists for `listing.token_id`.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists.
pub fn update_listing(env: &Env, listing: &Listing) -> Result<(), Error> {
    if !has_listing(env, listing.token_id) {
        return Err(Error::ListingNotFound);
    }
    save_listing(env, listing);
    Ok(())
}

/// Update only the `status` field of the listing for `token_id`.
///
/// Reads the current listing, changes its status, and writes it back.
/// The `active` convenience flag is kept in sync automatically: it is set to
/// `true` only when `new_status` is [`ListingStatus::Active`].
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`.
pub fn update_listing_status(
    env: &Env,
    token_id: TokenId,
    new_status: ListingStatus,
) -> Result<(), Error> {
    let mut listing = get_listing(env, token_id)?;
    listing.active = matches!(new_status, ListingStatus::Active);
    listing.status = new_status;
    save_listing(env, &listing);
    Ok(())
}

// ── Delete ────────────────────────────────────────────────────────────────────

/// Remove the listing record for `token_id`.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`.
pub fn remove_listing(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !has_listing(env, token_id) {
        return Err(Error::ListingNotFound);
    }
    env.storage()
        .persistent()
        .remove(&DataKey::Listing(token_id));
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::types::ListingStatus;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_listing(env: &Env, token_id: TokenId) -> Listing {
        Listing {
            token_id,
            seller: Address::generate(env),
            payment_asset: Address::generate(env),
            price: 1_000,
            created_at: 0,
            active: true,
            status: ListingStatus::Active,
            expires_at: 0,
            buyer: None,
            sold_at: None,
        }
    }

    // ── Store ──────────────────────────────────────────────────────────────────

    #[test]
    fn store_listing_persists_record() {
        let env = Env::default();
        let listing = make_listing(&env, 1);
        save_listing(&env, &listing);
        assert!(has_listing(&env, 1));
    }

    #[test]
    fn store_listing_overwrites_existing() {
        let env = Env::default();
        let mut listing = make_listing(&env, 1);
        save_listing(&env, &listing);
        listing.price = 2_000;
        save_listing(&env, &listing);
        assert_eq!(get_listing(&env, 1).unwrap().price, 2_000);
    }

    // ── Retrieve ──────────────────────────────────────────────────────────────

    #[test]
    fn get_listing_returns_stored_record() {
        let env = Env::default();
        let listing = make_listing(&env, 2);
        save_listing(&env, &listing);
        let retrieved = get_listing(&env, 2).unwrap();
        assert_eq!(retrieved.token_id, listing.token_id);
        assert_eq!(retrieved.price, listing.price);
        assert_eq!(retrieved.seller, listing.seller);
        assert_eq!(retrieved.status, ListingStatus::Active);
    }

    #[test]
    fn get_listing_missing_returns_listing_not_found() {
        let env = Env::default();
        assert_eq!(get_listing(&env, 99), Err(Error::ListingNotFound));
    }

    #[test]
    fn try_get_listing_returns_none_for_missing() {
        let env = Env::default();
        assert!(try_get_listing(&env, 99).is_none());
    }

    #[test]
    fn try_get_listing_returns_some_when_present() {
        let env = Env::default();
        let listing = make_listing(&env, 3);
        save_listing(&env, &listing);
        assert!(try_get_listing(&env, 3).is_some());
    }

    // ── Update ────────────────────────────────────────────────────────────────

    #[test]
    fn update_listing_changes_fields() {
        let env = Env::default();
        let mut listing = make_listing(&env, 4);
        save_listing(&env, &listing);
        listing.price = 9_999;
        update_listing(&env, &listing).unwrap();
        assert_eq!(get_listing(&env, 4).unwrap().price, 9_999);
    }

    #[test]
    fn update_listing_returns_error_when_missing() {
        let env = Env::default();
        let listing = make_listing(&env, 99);
        assert_eq!(update_listing(&env, &listing), Err(Error::ListingNotFound));
    }

    #[test]
    fn update_listing_status_changes_status() {
        let env = Env::default();
        let listing = make_listing(&env, 5);
        save_listing(&env, &listing);
        update_listing_status(&env, 5, ListingStatus::Cancelled).unwrap();
        assert_eq!(
            get_listing(&env, 5).unwrap().status,
            ListingStatus::Cancelled
        );
    }

    #[test]
    fn update_listing_status_sold() {
        let env = Env::default();
        let listing = make_listing(&env, 6);
        save_listing(&env, &listing);
        update_listing_status(&env, 6, ListingStatus::Sold).unwrap();
        assert_eq!(get_listing(&env, 6).unwrap().status, ListingStatus::Sold);
    }

    #[test]
    fn update_listing_status_missing_returns_error() {
        let env = Env::default();
        assert_eq!(
            update_listing_status(&env, 99, ListingStatus::Cancelled),
            Err(Error::ListingNotFound)
        );
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    #[test]
    fn remove_listing_deletes_record() {
        let env = Env::default();
        let listing = make_listing(&env, 7);
        save_listing(&env, &listing);
        remove_listing(&env, 7).unwrap();
        assert!(!has_listing(&env, 7));
    }

    #[test]
    fn remove_listing_returns_error_when_missing() {
        let env = Env::default();
        assert_eq!(remove_listing(&env, 99), Err(Error::ListingNotFound));
    }

    #[test]
    fn get_listing_after_remove_returns_not_found() {
        let env = Env::default();
        let listing = make_listing(&env, 8);
        save_listing(&env, &listing);
        remove_listing(&env, 8).unwrap();
        assert_eq!(get_listing(&env, 8), Err(Error::ListingNotFound));
    }

    // ── Round-trip ────────────────────────────────────────────────────────────

    #[test]
    fn listing_round_trip_preserves_all_fields() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let asset = Address::generate(&env);
        let listing = Listing {
            token_id: 42,
            seller: seller.clone(),
            payment_asset: asset.clone(),
            price: 5_000,
            created_at: 1_699_000_000,
            active: true,
            status: ListingStatus::Active,
            expires_at: 1_700_000_000,
            buyer: None,
            sold_at: None,
        };
        save_listing(&env, &listing);
        let loaded = get_listing(&env, 42).unwrap();
        assert_eq!(loaded.token_id, 42);
        assert_eq!(loaded.seller, seller);
        assert_eq!(loaded.price, 5_000);
        assert_eq!(loaded.payment_asset, asset);
        assert_eq!(loaded.expires_at, 1_700_000_000);
        assert_eq!(loaded.active, true);
        assert_eq!(loaded.status, ListingStatus::Active);
        assert_eq!(loaded.created_at, 1_699_000_000);
        assert!(loaded.buyer.is_none());
        assert!(loaded.sold_at.is_none());
    }
}
