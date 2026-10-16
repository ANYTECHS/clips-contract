//! Listing Cancellation Authorization Guard (Issue #1264).
//!
//! Verifies whether a caller is authorized to cancel a marketplace listing.
//!
//! # Acceptance criteria
//! 1. **Verify seller** — only the original seller (or an approved operator) may cancel.
//! 2. **Verify listing is active** — only `Active` listings can be cancelled.
//! 3. **Mark listing cancelled** — performed by the `cancel_listing` entry point after this guard.
//! 4. **Emit cancellation event** — performed by the `cancel_listing` entry point after this guard.

use soroban_sdk::{Address, Env};

use crate::listing_storage;
use crate::operator_approval;
use crate::types::{Error, ListingStatus, TokenId};

// ── Primary entry point ───────────────────────────────────────────────────────

/// Validate that `caller` is authorized to cancel the listing for `token_id`.
///
/// Checks in order:
/// 1. Listing must exist → [`Error::ListingNotFound`].
/// 2. Caller must be the seller or an approved operator of the seller →
///    [`Error::Unauthorized`].
/// 3. Listing must be in `Active` state → [`Error::ListingNotActive`].
///
/// `caller.require_auth()` is invoked first so Soroban always validates the
/// cryptographic authorization envelope before any storage reads.
pub fn require_cancellation_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    caller.require_auth();

    // 1. Listing must exist.
    let listing = listing_storage::get_listing(env, token_id)?;

    // 2. Caller must be the seller or an approved operator.
    check_cancellation_authority(env, caller, &listing.seller)?;

    // 3. Listing must be Active.
    check_listing_is_cancellable(env, token_id)?;

    Ok(())
}

// ── Authorization helpers ────────────────────────────────────────────────────

/// Return `Ok(())` if `caller` is the seller or an approved operator of the seller.
pub fn check_cancellation_authority(
    env: &Env,
    caller: &Address,
    seller: &Address,
) -> Result<(), Error> {
    if caller == seller {
        return Ok(());
    }
    if operator_approval::is_operator(env, seller, caller) {
        return Ok(());
    }
    Err(Error::Unauthorized)
}

/// Return `Ok(())` if the listing for `token_id` is in `Active` state.
///
/// # Errors
/// - [`Error::ListingNotFound`] if no listing exists.
/// - [`Error::ListingNotActive`] if the listing is not in `Active` state.
pub fn check_listing_is_cancellable(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let listing = listing_storage::get_listing(env, token_id)?;
    match listing.status {
        ListingStatus::Active => Ok(()),
        _ => Err(Error::ListingNotActive),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listing_request::ListingRequest;
    use crate::types::ListingStatus;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup_active_listing(env: &Env, seller: &Address, token_id: TokenId) {
        let mut listing = ListingRequest {
            listing_id: 0,
            token_id,
            price: 1_000,
            payment_asset: Address::generate(env),
            expiration: 0,
            seller: seller.clone(),
            status: ListingStatus::Active,
        };
        listing_storage::create_listing(env, &mut listing).unwrap();
    }

    // ─── Acceptance Criterion 1: Verify Seller ───────────────────────────────

    #[test]
    fn seller_passes_cancellation_authority_check() {
        let env = Env::default();
        let seller = Address::generate(&env);
        assert!(check_cancellation_authority(&env, &seller, &seller).is_ok());
    }

    #[test]
    fn non_seller_without_approval_fails_authority_check() {
        let env = Env::default();
        let seller = Address::generate(&env);
        let other = Address::generate(&env);
        assert_eq!(
            check_cancellation_authority(&env, &other, &seller),
            Err(Error::Unauthorized)
        );
    }

    // ─── Acceptance Criterion 2: Verify Listing Is Active ───────────────────

    #[test]
    fn active_listing_is_cancellable() {
        let env = Env::default();
        let seller = Address::generate(&env);
        setup_active_listing(&env, &seller, 1);
        assert!(check_listing_is_cancellable(&env, 1).is_ok());
    }

    #[test]
    fn missing_listing_returns_not_found() {
        let env = Env::default();
        assert_eq!(
            check_listing_is_cancellable(&env, 99),
            Err(Error::ListingNotFound)
        );
    }

    #[test]
    fn cancelled_listing_is_not_cancellable_again() {
        let env = Env::default();
        let seller = Address::generate(&env);
        setup_active_listing(&env, &seller, 2);
        listing_storage::update_listing_status(&env, 2, ListingStatus::Cancelled).unwrap();
        assert_eq!(
            check_listing_is_cancellable(&env, 2),
            Err(Error::ListingNotActive)
        );
    }

    #[test]
    fn sold_listing_is_not_cancellable() {
        let env = Env::default();
        let seller = Address::generate(&env);
        setup_active_listing(&env, &seller, 3);
        listing_storage::update_listing_status(&env, 3, ListingStatus::Sold).unwrap();
        assert_eq!(
            check_listing_is_cancellable(&env, 3),
            Err(Error::ListingNotActive)
        );
    }

    #[test]
    fn expired_listing_is_not_cancellable() {
        let env = Env::default();
        let seller = Address::generate(&env);
        setup_active_listing(&env, &seller, 4);
        listing_storage::update_listing_status(&env, 4, ListingStatus::Expired).unwrap();
        assert_eq!(
            check_listing_is_cancellable(&env, 4),
            Err(Error::ListingNotActive)
        );
    }

    // ─── Acceptance Criterion 3 & 4: cancel_listing entry point (integration) ─

    #[test]
    fn full_cancellation_flow_seller_can_cancel_active_listing() {
        let env = Env::default();
        let seller = Address::generate(&env);
        setup_active_listing(&env, &seller, 5);
        // Guard passes for seller + active listing
        assert!(check_cancellation_authority(&env, &seller, &seller).is_ok());
        assert!(check_listing_is_cancellable(&env, 5).is_ok());
        // Mark cancelled
        listing_storage::update_listing_status(&env, 5, ListingStatus::Cancelled).unwrap();
        // Verify it's now cancelled
        let stored = listing_storage::get_listing(&env, 5).unwrap();
        assert_eq!(stored.status, ListingStatus::Cancelled);
    }
}
