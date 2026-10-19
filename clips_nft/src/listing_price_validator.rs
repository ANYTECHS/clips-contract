//! Listing price validator (issue #1200).
//!
//! Validates NFT listing prices before they are accepted by the marketplace.
//! A valid price must be strictly positive and must not overflow the safe
//! arithmetic ceiling used throughout the contract.
//!
//! # Acceptance criteria
//! - Reject zero or negative prices.
//! - Enforce configured price constraints (maximum safe price).
//! - Prevent arithmetic overflow.
//! - Add boundary tests.
//!
//! # Errors
//! - [`Error::InvalidSalePrice`] — price is zero or negative.
//! - [`Error::PriceOverflow`]   — price exceeds [`MAX_LISTING_PRICE`].

use crate::types::Error;

/// Maximum safe listing price. Set to `i128::MAX / 2` to keep all downstream
/// arithmetic (royalty splits, fee calculations) well within `i128` bounds.
pub const MAX_LISTING_PRICE: i128 = i128::MAX / 2;

/// Validate an NFT listing price.
///
/// # Arguments
/// * `price` — Asking price in the payment asset's smallest unit.
///
/// # Errors
/// - [`Error::InvalidSalePrice`] when `price <= 0`.
/// - [`Error::PriceOverflow`] when `price > MAX_LISTING_PRICE`.
///
/// # Examples
/// ```rust,ignore
/// listing_price_validator::validate_listing_price(1_000_000)?;  // ok
/// listing_price_validator::validate_listing_price(0)?;          // Err(InvalidSalePrice)
/// listing_price_validator::validate_listing_price(-1)?;         // Err(InvalidSalePrice)
/// ```
pub fn validate_listing_price(price: i128) -> Result<(), Error> {
    if price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    if price > MAX_LISTING_PRICE {
        return Err(Error::PriceOverflow);
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Valid prices ─────────────────────────────────────────────────────────

    #[test]
    fn accepts_minimum_positive_price() {
        assert!(validate_listing_price(1).is_ok());
    }

    #[test]
    fn accepts_typical_price() {
        assert!(validate_listing_price(1_000_000).is_ok());
    }

    #[test]
    fn accepts_maximum_price() {
        assert!(validate_listing_price(MAX_LISTING_PRICE).is_ok());
    }

    // ── Boundary: zero and negative ──────────────────────────────────────────

    #[test]
    fn rejects_zero_price() {
        assert_eq!(validate_listing_price(0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn rejects_negative_one() {
        assert_eq!(validate_listing_price(-1), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn rejects_large_negative_price() {
        assert_eq!(
            validate_listing_price(i128::MIN),
            Err(Error::InvalidSalePrice)
        );
    }

    // ── Boundary: overflow ───────────────────────────────────────────────────

    #[test]
    fn rejects_price_above_maximum() {
        assert_eq!(
            validate_listing_price(MAX_LISTING_PRICE + 1),
            Err(Error::PriceOverflow)
        );
    }

    #[test]
    fn rejects_i128_max() {
        assert_eq!(
            validate_listing_price(i128::MAX),
            Err(Error::PriceOverflow)
        );
    }

    #[test]
    fn boundary_one_below_max_is_valid() {
        assert!(validate_listing_price(MAX_LISTING_PRICE - 1).is_ok());
    }
}
