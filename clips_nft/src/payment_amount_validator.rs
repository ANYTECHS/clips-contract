//! Payment amount validator (issue #1206).
//!
//! Validates payment amounts used by marketplace and royalty operations.
//!
//! Marketplace purchases, offers, and royalty distributions all move fungible
//! value between accounts. Before any transfer is authorised, the amount
//! must pass four independent gates:
//!
//! | Gate | Function | What it checks |
//! |------|----------|----------------|
//! | Non-zero | [`validate_non_zero`] | Amount is not zero (where prohibited) |
//! | Sign | [`validate_positive`] | Amount is strictly positive (> 0) |
//! | Overflow | [`check_add_overflow`] / [`check_mul_overflow`] | Arithmetic stays inside `i128` |
//! | Bounds | [`validate_amount_within_bounds`] | Amount ≤ `MAX_PAYMENT_AMOUNT` |
//!
//! # Acceptance criteria (issue #1206)
//!
//! 1. Reject zero amounts where prohibited — [`validate_non_zero`].
//! 2. Reject negative or invalid representations — [`validate_positive`].
//! 3. Prevent arithmetic overflow — [`check_add_overflow`], [`check_mul_overflow`].
//! 4. Add boundary tests — see the `tests` module.
//!
//! # Design
//!
//! Every function is pure (takes/returns plain values, no `Env` read) so the
//! validator is trivially testable and composes with the centralized
//! [`crate::validators::ValidationResult`] framework via
//! [`ValidationResult::from_result`].

use crate::types::Error;

/// Maximum allowed payment amount. Mirrors the bound enforced by the
/// marketplace listing/purchase validators (`MAX_LISTING_PRICE`,
/// `MAX_PURCHASE_PRICE`, `MAX_MARKETPLACE_PRICE`).
pub const MAX_PAYMENT_AMOUNT: i128 = i128::MAX / 2;

// ── Sign / non-zero gates ─────────────────────────────────────────────────────

/// Reject a zero amount.
///
/// Use this for contexts where a zero amount is nonsensical even though a
/// negative amount may already have been rejected upstream — e.g. a royalty
/// distribution that must pay *something*.
///
/// # Errors
/// Returns [`Error::InvalidSalePrice`] when `amount == 0`.
pub fn validate_non_zero(amount: i128) -> Result<(), Error> {
    if amount == 0 {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}

/// Reject non-positive (zero or negative) amounts.
///
/// The canonical "payment must be strictly positive" gate. Use this for
/// marketplace listing prices, offer amounts, and any payment that must move
/// value.
///
/// # Errors
/// Returns [`Error::InvalidSalePrice`] when `amount <= 0`.
pub fn validate_positive(amount: i128) -> Result<(), Error> {
    if amount <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}

/// Reject negative amounts but allow zero.
///
/// Use this for fields where zero is a valid "no-op" value (e.g. a platform fee
/// configured to 0 %) but a negative value is never acceptable.
///
/// # Errors
/// Returns [`Error::InvalidSalePrice`] when `amount < 0`.
pub fn validate_non_negative(amount: i128) -> Result<(), Error> {
    if amount < 0 {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}

// ── Bounds gate ──────────────────────────────────────────────────────────────

/// Validate that `amount` is inside `[0, MAX_PAYMENT_AMOUNT]`.
///
/// Combines the sign gate ([`validate_non_negative`]) with an upper-bound
/// check so callers do not have to run both. The upper bound guards against
/// prices that, after fees or royalties are added, would overflow `i128`.
///
/// # Errors
/// - [`Error::InvalidSalePrice`] — `amount < 0`.
/// - [`Error::PriceOverflow`]      — `amount > MAX_PAYMENT_AMOUNT`.
pub fn validate_amount_within_bounds(amount: i128) -> Result<(), Error> {
    validate_non_negative(amount)?;
    if amount > MAX_PAYMENT_AMOUNT {
        return Err(Error::PriceOverflow);
    }
    Ok(())
}

/// Validate a strictly-positive amount within bounds `[1, MAX_PAYMENT_AMOUNT]`.
///
/// Equivalent to [`validate_positive`] followed by the upper-bound check.
///
/// # Errors
/// - [`Error::InvalidSalePrice`] — `amount <= 0`.
/// - [`Error::PriceOverflow`]      — `amount > MAX_PAYMENT_AMOUNT`.
pub fn validate_positive_within_bounds(amount: i128) -> Result<(), Error> {
    validate_positive(amount)?;
    if amount > MAX_PAYMENT_AMOUNT {
        return Err(Error::PriceOverflow);
    }
    Ok(())
}

// ── Exact-match gate (payments) ───────────────────────────────────────────────

/// Validate that a payment `amount` exactly matches an `expected` price.
///
/// Used by the marketplace purchase path where the buyer must tender exactly
/// the listing price (no overpayment, no underpayment).
///
/// # Errors
/// - [`Error::InvalidSalePrice`]     — `expected` is not strictly positive.
/// - [`Error::IncorrectPaymentAmount`]— `amount != expected`.
pub fn validate_exact_payment(amount: i128, expected: i128) -> Result<(), Error> {
    validate_positive(expected)?;
    if amount != expected {
        return Err(Error::IncorrectPaymentAmount);
    }
    Ok(())
}

// ── Overflow gates ───────────────────────────────────────────────────────────

/// Add two `i128` values with an overflow check.
///
/// Returns the sum on success. On overflow (or underflow) returns
/// [`Error::RoyaltyOverflow`], the centralized error already used by the
/// royalty calculation path. Callers that need a domain-specific error can
/// `map_err` the result.
///
/// # Errors
/// Returns [`Error::RoyaltyOverflow`] when `a + b` cannot be represented in `i128`.
pub fn check_add_overflow(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_add(b).ok_or(Error::RoyaltyOverflow)
}

/// Subtract `sub` from `value` with an underflow check.
///
/// # Errors
/// Returns [`Error::RoyaltyOverflow`] when `value - sub` underflows `i128`.
pub fn check_sub_overflow(value: i128, sub: i128) -> Result<i128, Error> {
    value.checked_sub(sub).ok_or(Error::RoyaltyOverflow)
}

/// Multiply two `i128` values with an overflow check.
///
/// Used by royalty and platform-fee calculations (`price * bps / 10_000`) to
/// guard the intermediate product before the division.
///
/// # Errors
/// Returns [`Error::RoyaltyOverflow`] when `a * b` cannot be represented in `i128`.
pub fn check_mul_overflow(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_mul(b).ok_or(Error::RoyaltyOverflow)
}

/// Compute `price * bps / 10_000` with overflow protection.
///
/// This is the canonical safe path for basis-point prorated amounts (royalty,
/// platform fee). The intermediate `price * bps` product is checked for
/// overflow before the division, so a huge `price` paired with a large `bps`
/// fails loudly instead of wrapping.
///
/// # Errors
/// Returns [`Error::RoyaltyOverflow`] when `price * bps` overflows `i128`.
pub fn compute_bps_amount(price: i128, bps: i128) -> Result<i128, Error> {
    let product = check_mul_overflow(price, bps)?;
    Ok(product / 10_000)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::validators::ValidationResult;

    // ── validate_non_zero ────────────────────────────────────────────────────

    #[test]
    fn non_zero_rejects_zero() {
        assert_eq!(validate_non_zero(0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn non_zero_accepts_positive() {
        assert!(validate_non_zero(1).is_ok());
        assert!(validate_non_zero(1_000).is_ok());
    }

    #[test]
    fn non_zero_accepts_negative() {
        // validate_non_zero only rejects zero; sign is a separate gate.
        assert!(validate_non_zero(-1).is_ok());
    }

    // ── validate_positive ───────────────────────────────────────────────────

    #[test]
    fn positive_rejects_zero() {
        assert_eq!(validate_positive(0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn positive_rejects_negative() {
        assert_eq!(validate_positive(-1), Err(Error::InvalidSalePrice));
        assert_eq!(validate_positive(-1_000), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn positive_accepts_one() {
        assert!(validate_positive(1).is_ok());
    }

    // ── validate_non_negative ────────────────────────────────────────────────

    #[test]
    fn non_negative_accepts_zero() {
        assert!(validate_non_negative(0).is_ok());
    }

    #[test]
    fn non_negative_rejects_negative() {
        assert_eq!(validate_non_negative(-1), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn non_negative_accepts_positive() {
        assert!(validate_non_negative(42).is_ok());
    }

    // ── validate_amount_within_bounds ─────────────────────────────────────────

    #[test]
    fn bounds_rejects_negative() {
        assert_eq!(validate_amount_within_bounds(-1), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn bounds_accepts_zero() {
        assert!(validate_amount_within_bounds(0).is_ok());
    }

    #[test]
    fn bounds_accepts_max() {
        assert!(validate_amount_within_bounds(MAX_PAYMENT_AMOUNT).is_ok());
    }

    #[test]
    fn bounds_rejects_above_max() {
        assert_eq!(
            validate_amount_within_bounds(MAX_PAYMENT_AMOUNT + 1),
            Err(Error::PriceOverflow)
        );
        // i128::MAX is well above MAX_PAYMENT_AMOUNT and must overflow.
        assert_eq!(validate_amount_within_bounds(i128::MAX), Err(Error::PriceOverflow));
    }

    // ── validate_positive_within_bounds ───────────────────────────────────────

    #[test]
    fn positive_bounds_rejects_zero() {
        assert_eq!(validate_positive_within_bounds(0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn positive_bounds_rejects_negative() {
        assert_eq!(validate_positive_within_bounds(-5), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn positive_bounds_accepts_one_and_max() {
        assert!(validate_positive_within_bounds(1).is_ok());
        assert!(validate_positive_within_bounds(MAX_PAYMENT_AMOUNT).is_ok());
    }

    #[test]
    fn positive_bounds_rejects_above_max() {
        assert_eq!(
            validate_positive_within_bounds(MAX_PAYMENT_AMOUNT + 1),
            Err(Error::PriceOverflow)
        );
    }

    // ── validate_exact_payment ────────────────────────────────────────────────

    #[test]
    fn exact_payment_accepts_match() {
        assert!(validate_exact_payment(1_000, 1_000).is_ok());
    }

    #[test]
    fn exact_payment_rejects_underpayment() {
        assert_eq!(
            validate_exact_payment(999, 1_000),
            Err(Error::IncorrectPaymentAmount)
        );
    }

    #[test]
    fn exact_payment_rejects_overpayment() {
        assert_eq!(
            validate_exact_payment(1_001, 1_000),
            Err(Error::IncorrectPaymentAmount)
        );
    }

    #[test]
    fn exact_payment_rejects_zero_expected() {
        assert_eq!(validate_exact_payment(0, 0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn exact_payment_rejects_negative_expected() {
        assert_eq!(validate_exact_payment(10, -1), Err(Error::InvalidSalePrice));
    }

    // ── check_add_overflow ───────────────────────────────────────────────────

    #[test]
    fn add_no_overflow() {
        assert_eq!(check_add_overflow(2, 3), Ok(5));
    }

    #[test]
    fn add_overflow_rejected() {
        assert_eq!(check_add_overflow(i128::MAX, 1), Err(Error::RoyaltyOverflow));
    }

    #[test]
    fn add_underflow_rejected() {
        assert_eq!(check_add_overflow(i128::MIN, -1), Err(Error::RoyaltyOverflow));
    }

    #[test]
    fn add_zero_is_identity() {
        assert_eq!(check_add_overflow(1_000, 0), Ok(1_000));
    }

    // ── check_sub_overflow ───────────────────────────────────────────────────

    #[test]
    fn sub_no_underflow() {
        assert_eq!(check_sub_overflow(10, 3), Ok(7));
    }

    #[test]
    fn sub_underflow_rejected() {
        assert_eq!(check_sub_overflow(i128::MIN, 1), Err(Error::RoyaltyOverflow));
    }

    // ── check_mul_overflow ───────────────────────────────────────────────────

    #[test]
    fn mul_no_overflow() {
        assert_eq!(check_mul_overflow(100, 50), Ok(5_000));
    }

    #[test]
    fn mul_overflow_rejected() {
        // i128::MAX * 2 overflows.
        assert_eq!(check_mul_overflow(i128::MAX, 2), Err(Error::RoyaltyOverflow));
    }

    #[test]
    fn mul_by_zero_is_zero() {
        assert_eq!(check_mul_overflow(i128::MAX, 0), Ok(0));
    }

    // ── compute_bps_amount ────────────────────────────────────────────────────

    #[test]
    fn bps_amount_basic() {
        // 1000 stroops * 500 bps / 10000 = 50
        assert_eq!(compute_bps_amount(1_000, 500), Ok(50));
    }

    #[test]
    fn bps_amount_zero_price() {
        assert_eq!(compute_bps_amount(0, 500), Ok(0));
    }

    #[test]
    fn bps_amount_zero_bps() {
        assert_eq!(compute_bps_amount(1_000, 0), Ok(0));
    }

    #[test]
    fn bps_amount_overflow_rejected() {
        // price near i128::MAX * 500 bps overflows.
        assert_eq!(
            compute_bps_amount(i128::MAX, 500),
            Err(Error::RoyaltyOverflow)
        );
    }

    #[test]
    fn bps_amount_full_rate() {
        // 10000 bps = 100% → returns the full price.
        assert_eq!(compute_bps_amount(7_500, 10_000), Ok(7_500));
    }

    // ── Centralized-error integration ─────────────────────────────────────────

    #[test]
    fn validator_results_integrate_with_standardized_result() {
        // The pure validators compose with ValidationResult::from_result.
        let ok = ValidationResult::from_result(validate_positive(100));
        assert!(ok.is_valid());

        let err = ValidationResult::from_result(validate_positive(0));
        assert_eq!(err.error(), Some(Error::InvalidSalePrice));
    }

    // ── Boundary conditions regression coverage ──────────────────────────────

    #[test]
    fn boundary_one_is_accepted() {
        assert!(validate_positive(1).is_ok());
        assert!(validate_positive_within_bounds(1).is_ok());
        assert!(validate_exact_payment(1, 1).is_ok());
    }

    #[test]
    fn boundary_max_is_accepted() {
        assert!(validate_amount_within_bounds(MAX_PAYMENT_AMOUNT).is_ok());
        assert!(validate_positive_within_bounds(MAX_PAYMENT_AMOUNT).is_ok());
    }

    #[test]
    fn boundary_just_above_max_is_rejected() {
        assert_eq!(
            validate_amount_within_bounds(MAX_PAYMENT_AMOUNT + 1),
            Err(Error::PriceOverflow)
        );
    }
}
