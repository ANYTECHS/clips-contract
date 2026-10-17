//! Marketplace purchase validator.
//!
//! Implements all pre-purchase validation checks required before an NFT
//! listing can be completed.  Every acceptance criterion from issues
//! "Validate Buyer Payment Balance" and "Create Purchase Validator" is
//! handled here.
//!
//! # Validation order (short-circuit on first failure)
//! 1. Contract is not paused.
//! 2. Buyer is not the seller (no self-purchase).
//! 3. Listing exists and is in `Active` status.
//! 4. Listing has not expired.
//! 5. Payment asset matches the listing's required asset.
//! 6. Payment asset is supported by the contract.
//! 7. Payment amount equals the listing price.
//! 8. Buyer has sufficient balance to cover the payment amount.
//! 9. Seller still owns the NFT (ownership guard).

use soroban_sdk::{token, Address, Env};

use crate::marketplace::types::{Listing, ListingStatus};
use crate::pause_state;
use crate::payment_currency;
use crate::purchase_request::PurchaseRequest;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ── Core validator ────────────────────────────────────────────────────────────

/// Validate all preconditions for purchasing an NFT listing.
///
/// # Arguments
/// * `env`           — Contract execution environment.
/// * `buyer`         — Address of the prospective buyer.
/// * `listing`       — The [`Listing`] record being purchased.
/// * `payment_asset` — Asset contract the buyer will pay with.
/// * `amount`        — Amount the buyer intends to pay.
///
/// # Errors
/// | Error | Condition |
/// |-------|-----------|
/// | `ContractPaused`        | Contract is globally paused. |
/// | `SelfTransferNotAllowed`| Buyer and seller are the same address. |
/// | `ListingAlreadySold`    | Listing is in `Sold` status. |
/// | `ListingNotActive`      | Listing is `Cancelled`. |
/// | `ListingExpired`        | Listing is `Expired` or expiry timestamp has passed. |
/// | `PaymentAssetMismatch`  | `payment_asset` ≠ `listing.payment_asset`. |
/// | `UnsupportedAsset`      | `payment_asset` is not a registered currency. |
/// | `IncorrectPaymentAmount`| `amount` ≠ `listing.price`. |
/// | `InsufficientBalance`   | Buyer's on-chain balance is less than `amount`. |
/// | `Unauthorized`          | Seller no longer owns the NFT. |
pub fn validate_purchase(
    env: &Env,
    buyer: &Address,
    listing: &Listing,
    payment_asset: &Address,
    amount: i128,
) -> Result<(), Error> {
    // 1. Contract must not be paused.
    if pause_state::get_pause_state(env) {
        return Err(Error::ContractPaused);
    }

    // 2. Buyer must not be the seller.
    if buyer == &listing.seller {
        return Err(Error::SelfTransferNotAllowed);
    }

    // 3. Listing must be Active.
    match listing.status {
        ListingStatus::Active => {}
        ListingStatus::Sold => return Err(Error::ListingAlreadySold),
        ListingStatus::Cancelled => return Err(Error::ListingNotActive),
        ListingStatus::Expired => return Err(Error::ListingExpired),
    }

    // 4. Listing must not have passed its expiry timestamp.
    let now = env.ledger().timestamp();
    if listing.expires_at != 0 && listing.expires_at <= now {
        return Err(Error::ListingExpired);
    }

    // 5. Payment asset must match the one required by the listing.
    if payment_asset != &listing.payment_asset {
        return Err(Error::PaymentAssetMismatch);
    }

    // 6. Payment asset must be a supported currency.
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }

    // 7. Payment amount must exactly match the listing price.
    if amount != listing.price {
        return Err(Error::IncorrectPaymentAmount);
    }

    // 8. Buyer must have sufficient on-chain balance to cover the payment.
    let token_client = token::Client::new(env, payment_asset);
    let buyer_balance = token_client.balance(buyer);
    if buyer_balance < amount {
        return Err(Error::InsufficientBalance);
    }

    // 9. Seller must still own the NFT (guards against stale listings).
    token_owner_storage::verify_owner(env, listing.token_id, &listing.seller)?;

    Ok(())
}

// ── Convenience wrappers ──────────────────────────────────────────────────────

/// Validate a purchase request against the stored listing for `token_id`.
///
/// Loads the listing from storage, runs all validation checks via
/// [`validate_purchase`], and returns the listing on success.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for `token_id`,
/// or any of the errors from [`validate_purchase`].
pub fn validate_purchase_for_token(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    payment_asset: &Address,
    amount: i128,
) -> Result<Listing, Error> {
    let listing = crate::marketplace::listing_storage::get_listing(env, token_id)?;
    validate_purchase(env, buyer, &listing, payment_asset, amount)?;
    Ok(listing)
}

/// Validate using a [`PurchaseRequest`] struct.
///
/// This variant accepts a structured request, loads the appropriate listing,
/// and delegates to [`validate_purchase`].  The `max_price` field is treated
/// as the exact payment amount.
///
/// # Errors
/// Returns [`Error::ListingNotFound`] if no listing exists for the requested
/// listing, or any of the errors from [`validate_purchase`].
pub fn validate_purchase_request(
    env: &Env,
    buyer: &Address,
    request: &PurchaseRequest,
) -> Result<Listing, Error> {
    let listing =
        crate::marketplace::listing_storage::get_listing(env, request.listing_id as TokenId)?;
    validate_purchase(env, buyer, &listing, &request.payment_asset, request.max_price)?;
    Ok(listing)
}
