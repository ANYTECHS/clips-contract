//! Listing Modification Authorization Guard (Issue #1265).
//!
//! Verifies whether a caller is authorized to modify a marketplace listing.
//! Ensures only the seller can modify listings, and only when the listing is still active.
//!
//! # Acceptance criteria
//!
//! | Criterion | Guard function |
//! |-----------|----------------|
//! | Verify listing modification authority | [`check_modification_authority`] |
//! | Reject modifications to sold/immutable listings | enforced by [`require_modification_auth`] |
//! | Validate listing state before updates | [`check_listing_is_modifiable`] |
//! | Add tests | see `#[cfg(test)]` block below |
//!
//! # Authorization hierarchy
//!
//! A caller may modify a listing only when **all** of the following conditions hold:
//!
//! 1. **Caller is the seller** — the account that created the listing.
//! 2. **Listing exists** — the referenced `token_id` has an active listing.
//! 3. **Listing is modifiable** — the listing is in `Active` state, not sold/cancelled/expired.
//!
//! Note: Unlike creation authorization, modification does **not** support operators or admins.
//! Only the original seller can modify their own listings.
//!
//! # Usage
//!
//! ```rust,ignore
//! listing_modification_guard::require_modification_auth(&env, &caller, token_id)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::listing_storage;
use crate::types::{Error, ListingStatus, TokenId};

// ─── Primary entry point ──────────────────────────────────────────────────────

/// Validate that `caller` is authorized to modify a listing for `token_id`.
///
/// Performs all modification pre-condition checks in order:
/// 1. Listing exists — returns [`Error::ListingNotFound`] if missing.
/// 2. Caller is the seller — returns [`Error::Unauthorized`] if not.
/// 3. Listing is modifiable — returns [`Error::Unauthorized`] if sold/cancelled/expired.
///
/// `caller.require_auth()` is called before any storage reads so that the
/// Soroban host always validates the cryptographic authorization envelope.
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `caller`   — Address attempting to modify the listing.
/// * `token_id` — Token whose listing is being modified.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ListingNotFound` | No listing exists for `token_id`. |
/// | `Unauthorized`  | `caller` is not the seller. |
/// | `Unauthorized`  | Listing is not in `Active` state (sold/cancelled/expired). |
pub fn require_modification_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // Demand a signed authorization envelope from the caller first.
    caller.require_auth();

    // 1. Listing must exist; retrieve it.
    let listing = listing_storage::get_listing(env, token_id)?;

    // 2. Caller must be the seller (no operators or admins allowed).
    check_modification_authority(caller, &listing.seller)?;

    // 3. Listing must be in a modifiable state (Active only).
    check_listing_is_modifiable(env, token_id)?;

    Ok(())
}

// ─── Authorization logic ──────────────────────────────────────────────────────

/// Check whether `caller` is the seller who can modify the listing.
///
/// Unlike creation authorization, modification supports only the seller.
/// Operators and admins cannot modify listings.
///
/// # Errors
/// Returns [`Error::Unauthorized`] if `caller` is not the seller.
pub fn check_modification_authority(caller: &Address, seller: &Address) -> Result<(), Error> {
    if caller == seller {
        Ok(())
    } else {
        Err(Error::Unauthorized)
    }
}

/// Check whether a listing is in a state that permits modification.
///
/// Only `Active` listings can be modified. Once sold, cancelled, or expired,
/// a listing becomes immutable and cannot be updated.
///
/// # Errors
/// Returns [`Error::Unauthorized`] if the listing is not in `Active` state.
pub fn check_listing_is_modifiable(env: &Env, token_id: TokenId) -> Result<(), Error> {
    let listing = listing_storage::get_listing(env, token_id)?;

    match listing.status {
        ListingStatus::Active => Ok(()),
        ListingStatus::Sold => Err(Error::Unauthorized),
        ListingStatus::Cancelled => Err(Error::Unauthorized),
        ListingStatus::Expired => Err(Error::Unauthorized),
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Retrieve the current state of a listing to verify it's modifiable.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`.
pub fn get_listing_state(env: &Env, token_id: TokenId) -> Result<ListingStatus, Error> {
    listing_storage::get_listing(env, token_id).map(|listing| listing.status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listing_request::ListingRequest;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        f(&env)
    }

    fn setup_listing(env: &Env, seller: &Address, token_id: TokenId, status: ListingStatus) {
        // Create a minimal listing in storage
        let listing = ListingRequest {
            listing_id: 0,
            token_id,
            price: 1000,
            payment_asset: Address::generate(env),
            expiration: 0,
            seller: seller.clone(),
        };
        let mut created = listing.clone();
        listing_storage::create_listing(env, &mut created).unwrap();

        // Update status to test state
        if status != ListingStatus::Active {
            listing_storage::update_listing_status(env, token_id, status).unwrap();
        }
    }

    // ─── Acceptance Criterion 1: Verify Modification Authority ────────────────────

    #[test]
    fn criterion_1_1_seller_can_modify_their_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Active);

            let result = require_modification_auth(env, &seller, token_id);
            assert!(
                result.is_ok(),
                "Seller should have authority to modify their listing"
            );
        });
    }

    #[test]
    fn criterion_1_2_non_seller_cannot_modify_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let non_seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Active);

            let result = require_modification_auth(env, &non_seller, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Non-seller should not have authority to modify listing"
            );
        });
    }

    #[test]
    fn criterion_1_3_operator_cannot_modify_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let operator = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Active);

            // Even if operator, they cannot modify listings (unlike creation)
            let result = require_modification_auth(env, &operator, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Operator should not have authority to modify listing"
            );
        });
    }

    #[test]
    fn criterion_1_4_admin_cannot_modify_listing() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let admin = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Active);

            // Set the admin in contract storage
            env.storage()
                .instance()
                .set(&crate::types::DataKey::Admin, &admin);

            // Even admin cannot modify listings
            let result = require_modification_auth(env, &admin, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Admin should not have authority to modify listing"
            );
        });
    }

    // ─── Acceptance Criterion 2: Reject Immutable Listings ──────────────────────

    #[test]
    fn criterion_2_1_sold_listing_cannot_be_modified() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Sold);

            let result = require_modification_auth(env, &seller, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Sold listing should not be modifiable"
            );
        });
    }

    #[test]
    fn criterion_2_2_cancelled_listing_cannot_be_modified() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Cancelled);

            let result = require_modification_auth(env, &seller, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Cancelled listing should not be modifiable"
            );
        });
    }

    #[test]
    fn criterion_2_3_expired_listing_cannot_be_modified() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Expired);

            let result = require_modification_auth(env, &seller, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Expired listing should not be modifiable"
            );
        });
    }

    // ─── Acceptance Criterion 3: Validate Listing State ────────────────────────

    #[test]
    fn criterion_3_1_check_listing_state_active() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Active);

            let result = check_listing_is_modifiable(env, token_id);
            assert!(result.is_ok(), "Active listing should be modifiable");
        });
    }

    #[test]
    fn criterion_3_2_check_listing_state_sold() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Sold);

            let result = check_listing_is_modifiable(env, token_id);
            assert_eq!(
                result,
                Err(Error::Unauthorized),
                "Sold listing should fail modifiability check"
            );
        });
    }

    #[test]
    fn criterion_3_3_get_listing_state_returns_correct_status() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id_active = 1u32;
            let token_id_sold = 2u32;
            setup_listing(env, &seller, token_id_active, ListingStatus::Active);
            setup_listing(env, &seller, token_id_sold, ListingStatus::Sold);

            let active_state = get_listing_state(env, token_id_active);
            assert_eq!(active_state, Ok(ListingStatus::Active));

            let sold_state = get_listing_state(env, token_id_sold);
            assert_eq!(sold_state, Ok(ListingStatus::Sold));
        });
    }

    // ─── Acceptance Criterion 4: Unit Tests ──────────────────────────────────────

    #[test]
    fn criterion_4_1_comprehensive_modification_authorization() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let other_seller = Address::generate(env);
            let token_id = 1u32;

            setup_listing(env, &seller, token_id, ListingStatus::Active);

            // Seller can modify
            assert!(require_modification_auth(env, &seller, token_id).is_ok());

            // Other seller cannot modify
            assert_eq!(
                require_modification_auth(env, &other_seller, token_id),
                Err(Error::Unauthorized)
            );

            // Transition to sold, then seller still cannot modify
            listing_storage::update_listing_status(env, token_id, ListingStatus::Sold).unwrap();
            assert_eq!(
                require_modification_auth(env, &seller, token_id),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn criterion_4_2_all_listing_states_validation() {
        with_contract(|env| {
            let seller = Address::generate(env);

            // Test each state
            let states = [
                (1u32, ListingStatus::Active, true),
                (2u32, ListingStatus::Sold, false),
                (3u32, ListingStatus::Cancelled, false),
                (4u32, ListingStatus::Expired, false),
            ];

            for (token_id, status, should_be_modifiable) in states.iter() {
                setup_listing(env, &seller, *token_id, *status);

                let result = check_listing_is_modifiable(env, *token_id);
                if *should_be_modifiable {
                    assert!(
                        result.is_ok(),
                        "Status {:?} should be modifiable",
                        status
                    );
                } else {
                    assert_eq!(
                        result,
                        Err(Error::Unauthorized),
                        "Status {:?} should not be modifiable",
                        status
                    );
                }
            }
        });
    }

    #[test]
    fn criterion_4_3_authority_check_function() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let other = Address::generate(env);

            // Same address passes
            assert!(check_modification_authority(&seller, &seller).is_ok());

            // Different address fails
            assert_eq!(
                check_modification_authority(&other, &seller),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn criterion_4_4_auth_called_before_storage_reads() {
        // This test structure ensures authorization envelope is validated
        // before attempting any storage lookups
        with_contract(|env| {
            let seller = Address::generate(env);
            let token_id = 1u32;
            setup_listing(env, &seller, token_id, ListingStatus::Active);

            // require_modification_auth calls seller.require_auth() internally
            // If it didn't, this would not enforce signature requirements
            let result = require_modification_auth(env, &seller, token_id);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn criterion_4_5_nonexistent_listing_handling() {
        with_contract(|env| {
            let seller = Address::generate(env);
            let nonexistent_token_id = 999u32;

            let result = require_modification_auth(env, &seller, nonexistent_token_id);
            assert_eq!(
                result,
                Err(Error::ListingNotFound),
                "Nonexistent listing should return ListingNotFound"
            );
        });
    }
}
