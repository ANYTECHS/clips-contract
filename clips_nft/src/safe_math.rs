//! Safe arithmetic helpers for royalty calculations.
//!
//! # Overflow protection
//!
//! Royalty amounts are computed using 7-decimal scaling to support fractional
//! royalties on assets with 7 decimal places (e.g. Stellar/SEP-0041 tokens):
//!
//! ```text
//! scaled     = sale_price × basis_points × ASSET_SCALE
//! royalty    = (scaled + 5_000) / 10_000 / ASSET_SCALE
//! ```
//!
//! Where `ASSET_SCALE = 10_000_000` (10^7, matching Stellar's 7-decimal precision).
//!
//! This two-step approach preserves sub-unit precision that would otherwise be
//! lost to integer truncation when `sale_price` is small relative to `10_000`.
//!
//! # Safe price limits
//!
//! The pre-check guards against overflow:
//! `sale_price ≤ i128::MAX / (10_000 × ASSET_SCALE)`
//!
//! In practice this is still astronomically large (~1.7 × 10²⁷ stroops),
//! far beyond any realistic Stellar transaction value.
//!
//! ## Basis points range
//!
//! - Valid range: 0–10,000 basis points (0%–100%)
//! - 1 basis point = 0.01%
//!
//! ## Zero royalty transactions (issue #801)
//!
//! A zero royalty rate (`basis_points == 0`) is detected up-front and returns
//! `0` so payment operations can be skipped entirely.
//!
//! # Error handling
//!
//! - [`Error::InvalidSalePrice`] — Returned when `sale_price ≤ 0`
//! - [`Error::RoyaltyOverflow`]  — Returned when calculation would overflow

use crate::Error;

/// 7-decimal scaling factor matching Stellar SEP-0041 asset precision.
pub const ASSET_SCALE: i128 = 10_000_000;

/// Return `true` when the royalty rate is zero (issue #801).
///
/// Zero-royalty configurations must short-circuit payment processing.
pub fn is_zero_royalty_bps(basis_points: u32) -> bool {
    basis_points == 0
}

/// Compute royalty amount with 7-decimal precision to support fractional amounts.
///
/// Formula:
/// ```text
/// royalty = (sale_price × basis_points × ASSET_SCALE + 5_000) / 10_000 / ASSET_SCALE
/// ```
///
/// # Arguments
/// * `sale_price`   — Sale price in the asset's smallest unit. Must be > 0.
/// * `basis_points` — Royalty rate in basis points (1 bp = 0.01 %). Range: 0–10 000.
///
/// # Errors
/// * [`Error::InvalidSalePrice`] — `sale_price` ≤ 0.
/// * [`Error::RoyaltyOverflow`]  — arithmetic would overflow.
pub fn safe_royalty_amount(sale_price: i128, basis_points: u32) -> Result<i128, Error> {
    if sale_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    // Zero royalty rate: nothing is owed, skip the calculation entirely.
    if is_zero_royalty_bps(basis_points) {
        return Ok(0);
    }
    // Pre-check: sale_price × 10_000 × ASSET_SCALE must fit in i128.
    if sale_price > i128::MAX / (10_000 * ASSET_SCALE) {
        return Err(Error::RoyaltyOverflow);
    }
    let scaled = sale_price
        .checked_mul(basis_points as i128)
        .ok_or(Error::RoyaltyOverflow)?
        .checked_mul(ASSET_SCALE)
        .ok_or(Error::RoyaltyOverflow)?
        .checked_add(5_000)
        .ok_or(Error::RoyaltyOverflow)?;
    Ok(scaled / 10_000 / ASSET_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Zero royalty (issue #801) ────────────────────────────────────────────

    #[test]
    fn zero_royalty_returns_zero_amount() {
        assert_eq!(safe_royalty_amount(1_000_000, 0).unwrap(), 0);
    }

    #[test]
    fn zero_royalty_detected() {
        assert!(is_zero_royalty_bps(0));
        assert!(!is_zero_royalty_bps(1));
        assert!(!is_zero_royalty_bps(10_000));
    }
}