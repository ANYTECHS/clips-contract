//! Listing Cancellation Guard — authorization check for marketplace listing cancellations.
//!
//! Verifies that the caller is authorized to cancel a marketplace listing.
//! A cancellation is permitted when:
//!   1. A listing exists for `token_id`.
//!   2. The listing is in `Active` status (not already sold or cancelled).
//!   3. The caller is the listing's seller, an approved operator, or the contract admin.

use soroban_sdk::{Address, Env};

use crate::types::{Error, TokenId};

/// Verify that `caller` is authorized to cancel the listing for `token_id`.
///
/// Delegates to the full [`crate::marketplace::listing_validator::cancel_listing`]
/// which enforces all invariants in one step.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `TokenNotFound` | No listing exists for `token_id`. |
/// | `ListingNotActive` | Listing is already sold or cancelled. |
/// | `Unauthorized` | Caller is not the seller, an approved operator, or admin. |
pub fn require_cancellation_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    crate::marketplace::listing_validator::cancel_listing(env, caller, token_id)
}
