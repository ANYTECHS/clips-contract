//! Offer request validator (issue #1203).
//!
//! Validates offers submitted for NFTs before they are persisted.
//!
//! An offer represents a buyer's standing bid on an NFT. Before an offer
//! record is written to storage, every field must pass five independent
//! gates:
//!
//! | Gate | Function | What it checks |
//! |------|----------|----------------|
//! | NFT existence | [`validate_nft_exists`] | Token has an ownership record |
//! | Amount | [`validate_offer_amount`] | Offer price is strictly positive and within bounds |
//! | Buyer | [`validate_buyer`] | Buyer is not the contract, not blacklisted, not the owner |
//! | Expiration | [`validate_offer_expiration`] | Expiry is in the future (or `0` = never) |
//! | Full request | [`validate_offer_request`] | All of the above + supported asset |
//!
//! # Acceptance criteria (issue #1203)
//!
//! 1. Validate NFT existence — [`validate_nft_exists`].
//! 2. Validate offer amount — [`validate_offer_amount`].
//! 3. Validate buyer — [`validate_buyer`].
//! 4. Validate offer expiration — [`validate_offer_expiration`].
//! 5. Reject invalid offers — every gate returns a centralized [`Error`].
//!
//! # Errors
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::TokenNotFound`]    | NFT does not exist. |
//! | [`Error::InvalidSalePrice`] | Offer amount is zero or negative. |
//! | [`Error::PriceOverflow`]     | Offer amount exceeds `MAX_PAYMENT_AMOUNT`. |
//! | [`Error::InvalidAddress`]    | Buyer is the contract's own address or is blacklisted. |
//! | [`Error::SelfTransferNotAllowed`] | Buyer is the current NFT owner. |
//! | [`Error::UnsupportedAsset`] | Payment asset is not a registered currency. |
//! | [`Error::OfferExpired`]     | Offer expiry timestamp is in the past. |

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::payment_amount_validator::{validate_positive_within_bounds, MAX_PAYMENT_AMOUNT};
use crate::payment_currency;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// Request struct describing a buyer's offer on an NFT.
///
/// Mirrors the fields required to validate an offer submission without
/// coupling callers to the persisted [`crate::marketplace::types::Offer`]
/// record (which carries `offer_id`, `status`, and `created_at` assigned at
/// persistence time).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfferRequest {
    /// Token the buyer is offering on.
    pub token_id: TokenId,
    /// Buyer placing the offer.
    pub buyer: Address,
    /// Offered amount in the payment asset's smallest unit.
    pub amount: i128,
    /// Payment asset contract address.
    pub payment_asset: Address,
    /// Unix expiration timestamp; `0` means the offer never expires.
    pub expires_at: u64,
}

// ── NFT existence gate ─────────────────────────────────────────────────────────

/// Validate that the NFT identified by `token_id` exists on-chain.
///
/// Existence is defined as "has an ownership record", the same definition used
/// by [`crate::token_id_validator::validate_token_id`].
///
/// # Errors
/// - [`Error::TokenNotFound`] — no ownership record exists for `token_id`.
pub fn validate_nft_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !token_owner_storage::has_owner(env, token_id) {
        return Err(Error::TokenNotFound);
    }
    Ok(())
}

// ── Offer amount gate ─────────────────────────────────────────────────────────

/// Validate the offer `amount`.
///
/// The amount must be strictly positive (`> 0`) and must not exceed
/// [`MAX_PAYMENT_AMOUNT`] so that downstream fee/royalty arithmetic cannot
/// overflow `i128`.
///
/// # Errors
/// - [`Error::InvalidSalePrice`] — `amount <= 0`.
/// - [`Error::PriceOverflow`]      — `amount > MAX_PAYMENT_AMOUNT`.
pub fn validate_offer_amount(amount: i128) -> Result<(), Error> {
    validate_positive_within_bounds(amount)
}

// ── Buyer gate ─────────────────────────────────────────────────────────────────

/// Validate `buyer` as an eligible offeror for `token_id`.
///
/// A buyer is eligible when:
/// 1. They are not the contract's own address.
/// 2. They are not blacklisted.
/// 3. They are not the current owner of `token_id` (no self-offers).
///
/// # Errors
/// - [`Error::InvalidAddress`]          — `buyer` is the contract's address or is blacklisted.
/// - [`Error::TokenNotFound`]           — `token_id` has no ownership record (when owner lookup runs).
/// - [`Error::SelfTransferNotAllowed`]  — `buyer` is the current owner of `token_id`.
pub fn validate_buyer(env: &Env, buyer: &Address, token_id: TokenId) -> Result<(), Error> {
    if *buyer == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    if blacklist::is_blacklisted(env, buyer) {
        return Err(Error::InvalidAddress);
    }
    // Reject self-offers: the owner offering on their own listed token is
    // nonsensical. Use a soft probe so a missing token still fails NFT existence
    // via validate_nft_exists downstream rather than here.
    if token_owner_storage::has_owner(env, token_id) {
        if let Ok(owner) = token_owner_storage::get_owner(env, token_id) {
            if *buyer == owner {
                return Err(Error::SelfTransferNotAllowed);
            }
        }
    }
    Ok(())
}

// ── Expiration gate ───────────────────────────────────────────────────────────

/// Validate the offer expiration timestamp.
///
/// `expires_at == 0` means the offer never expires and is always accepted.
/// Any non-zero `expires_at` must be strictly greater than the current ledger
/// timestamp.
///
/// # Errors
/// - [`Error::OfferExpired`] — `expires_at != 0 && expires_at <= now`.
pub fn validate_offer_expiration(env: &Env, expires_at: u64) -> Result<(), Error> {
    if expires_at == 0 {
        return Ok(());
    }
    let now = env.ledger().timestamp();
    if expires_at <= now {
        return Err(Error::OfferExpired);
    }
    Ok(())
}

// ── Payment asset gate ───────────────────────────────────────────────────────

/// Validate that `payment_asset` is a supported currency.
///
/// # Errors
/// - [`Error::UnsupportedAsset`] — asset is not registered as a supported currency.
pub fn validate_offer_asset(env: &Env, payment_asset: &Address) -> Result<(), Error> {
    if !payment_currency::is_supported(env, payment_asset) {
        return Err(Error::UnsupportedAsset);
    }
    Ok(())
}

// ── Full request validator ────────────────────────────────────────────────────

/// Validate a complete [`OfferRequest`] before it is persisted.
///
/// Runs every gate in order:
/// 1. NFT existence ([`validate_nft_exists`])
/// 2. Buyer eligibility ([`validate_buyer`])
/// 3. Offer amount ([`validate_offer_amount`])
/// 4. Payment asset ([`validate_offer_asset`])
/// 5. Offer expiration ([`validate_offer_expiration`])
///
/// Short-circuits on the first failure and returns the centralized [`Error`].
///
/// # Errors
/// Aggregates the errors of every gate above; see their documentation.
pub fn validate_offer_request(env: &Env, request: &OfferRequest) -> Result<(), Error> {
    validate_nft_exists(env, request.token_id)?;
    validate_buyer(env, &request.buyer, request.token_id)?;
    validate_offer_amount(request.amount)?;
    validate_offer_asset(env, &request.payment_asset)?;
    validate_offer_expiration(env, request.expires_at)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    fn setup_asset(env: &Env) -> Address {
        let asset = Address::generate(env);
        payment_currency::add_currency(env, asset.clone()).unwrap();
        asset
    }

    fn make_request(
        env: &Env,
        token_id: TokenId,
        buyer: &Address,
        amount: i128,
        asset: &Address,
        expires_at: u64,
    ) -> OfferRequest {
        OfferRequest {
            token_id,
            buyer: buyer.clone(),
            amount,
            payment_asset: asset.clone(),
            expires_at,
        }
    }

    // ── validate_nft_exists ───────────────────────────────────────────────────

    #[test]
    fn nft_exists_accepts_existing_token() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(validate_nft_exists(env, 1).is_ok());
        });
    }

    #[test]
    fn nft_exists_rejects_missing_token() {
        with_contract(|env| {
            assert_eq!(validate_nft_exists(env, 999), Err(Error::TokenNotFound));
        });
    }

    // ── validate_offer_amount ────────────────────────────────────────────────

    #[test]
    fn offer_amount_accepts_positive() {
        assert!(validate_offer_amount(1).is_ok());
        assert!(validate_offer_amount(1_000).is_ok());
        assert!(validate_offer_amount(MAX_PAYMENT_AMOUNT).is_ok());
    }

    #[test]
    fn offer_amount_rejects_zero() {
        assert_eq!(validate_offer_amount(0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn offer_amount_rejects_negative() {
        assert_eq!(validate_offer_amount(-1), Err(Error::InvalidSalePrice));
        assert_eq!(validate_offer_amount(-1_000), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn offer_amount_rejects_above_max() {
        assert_eq!(
            validate_offer_amount(MAX_PAYMENT_AMOUNT + 1),
            Err(Error::PriceOverflow)
        );
    }

    // ── validate_buyer ──────────────────────────────────────────────────────

    #[test]
    fn buyer_accepts_external_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(validate_buyer(env, &buyer, 1).is_ok());
        });
    }

    #[test]
    fn buyer_rejects_contract_self_address() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_buyer(env, &contract_addr, 1),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn buyer_rejects_blacklisted() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            blacklist::add_wallet(env, &buyer);
            assert_eq!(
                validate_buyer(env, &buyer, 1),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn buyer_rejects_current_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                validate_buyer(env, &owner, 1),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    // ── validate_offer_expiration ─────────────────────────────────────────────

    #[test]
    fn expiration_zero_never_expires() {
        with_contract(|env| {
            assert!(validate_offer_expiration(env, 0).is_ok());
        });
    }

    #[test]
    fn expiration_future_accepted() {
        with_contract(|env| {
            env.ledger().set_timestamp(1_000);
            assert!(validate_offer_expiration(env, 5_000).is_ok());
        });
    }

    #[test]
    fn expiration_past_rejected() {
        with_contract(|env| {
            env.ledger().set_timestamp(5_000);
            assert_eq!(
                validate_offer_expiration(env, 1_000),
                Err(Error::OfferExpired)
            );
        });
    }

    #[test]
    fn expiration_equal_to_now_rejected() {
        with_contract(|env| {
            env.ledger().set_timestamp(5_000);
            assert_eq!(
                validate_offer_expiration(env, 5_000),
                Err(Error::OfferExpired)
            );
        });
    }

    // ── validate_offer_asset ────────────────────────────────────────────────

    #[test]
    fn asset_accepts_supported_currency() {
        with_contract(|env| {
            let asset = setup_asset(env);
            assert!(validate_offer_asset(env, &asset).is_ok());
        });
    }

    #[test]
    fn asset_rejects_unsupported_currency() {
        with_contract(|env| {
            let asset = Address::generate(env);
            assert_eq!(
                validate_offer_asset(env, &asset),
                Err(Error::UnsupportedAsset)
            );
        });
    }

    // ── validate_offer_request (full pipeline) ─────────────────────────────────

    #[test]
    fn valid_offer_request_passes() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            let req = make_request(env, 1, &buyer, 1_000, &asset, 0);
            assert!(validate_offer_request(env, &req).is_ok());
        });
    }

    #[test]
    fn request_rejects_missing_nft() {
        with_contract(|env| {
            let buyer = Address::generate(env);
            let asset = setup_asset(env);
            let req = make_request(env, 999, &buyer, 1_000, &asset, 0);
            assert_eq!(
                validate_offer_request(env, &req),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn request_rejects_blacklisted_buyer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            blacklist::add_wallet(env, &buyer);
            let asset = setup_asset(env);
            let req = make_request(env, 1, &buyer, 1_000, &asset, 0);
            assert_eq!(
                validate_offer_request(env, &req),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn request_rejects_owner_buyer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            let req = make_request(env, 1, &owner, 1_000, &asset, 0);
            assert_eq!(
                validate_offer_request(env, &req),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn request_rejects_zero_amount() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            let req = make_request(env, 1, &buyer, 0, &asset, 0);
            assert_eq!(
                validate_offer_request(env, &req),
                Err(Error::InvalidSalePrice)
            );
        });
    }

    #[test]
    fn request_rejects_unsupported_asset() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = Address::generate(env); // not registered
            let req = make_request(env, 1, &buyer, 1_000, &asset, 0);
            assert_eq!(
                validate_offer_request(env, &req),
                Err(Error::UnsupportedAsset)
            );
        });
    }

    #[test]
    fn request_rejects_expired_offer() {
        with_contract(|env| {
            env.ledger().set_timestamp(5_000);
            let owner = Address::generate(env);
            let buyer = Address::generate(env);
            setup_token(env, 1, &owner);
            let asset = setup_asset(env);
            let req = make_request(env, 1, &buyer, 1_000, &asset, 1_000);
            assert_eq!(
                validate_offer_request(env, &req),
                Err(Error::OfferExpired)
            );
        });
    }

    // ── Centralized-error integration ─────────────────────────────────────────

    #[test]
    fn offer_errors_are_centralized_error_variants() {
        // Every gate returns crate::types::Error variants (not custom error types),
        // satisfying the "centralized errors" acceptance criterion.
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            let buyer = Address::generate(env);
            let asset = setup_asset(env);

            // Each distinct failure produces a distinct centralized error.
            let missing = make_request(env, 999, &buyer, 1_000, &asset, 0);
            assert_eq!(validate_offer_request(env, &missing), Err(Error::TokenNotFound));

            let zero = make_request(env, 1, &buyer, 0, &asset, 0);
            assert_eq!(validate_offer_request(env, &zero), Err(Error::InvalidSalePrice));

            let bad_asset = make_request(env, 1, &buyer, 1_000, &Address::generate(env), 0);
            assert_eq!(validate_offer_request(env, &bad_asset), Err(Error::UnsupportedAsset));
        });
    }
}
