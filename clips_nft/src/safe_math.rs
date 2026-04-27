//! Safe arithmetic helpers for royalty calculations.
//!
//! # Overflow protection
//!
//! Royalty amounts are computed as:
//!
//! ```text
//! royalty_amount = (sale_price × basis_points + 5_000) / 10_000
//! ```
//!
//! `sale_price` is an `i128`. The multiplication `sale_price × basis_points`
//! can overflow when `sale_price` is very large. This module guards against
//! that by:
//!
//! 1. Rejecting any `sale_price > i128::MAX / 10_000` before multiplying.
//! 2. Using `checked_mul` / `checked_add` so any residual overflow returns
//!    `Err` rather than wrapping silently.
//!
//! The maximum safe sale price is `i128::MAX / 10_000 ≈ 1.7 × 10³⁴` stroops,
//! which is astronomically larger than any realistic Stellar transaction value.

use crate::Error;

/// Compute `(sale_price × basis_points + 5_000) / 10_000` with overflow protection.
///
/// # Arguments
/// * `sale_price`   — Sale price in the asset's smallest unit. Must be > 0.
/// * `basis_points` — Royalty rate in basis points (1 bp = 0.01 %). Range: 0–10 000.
///
/// # Errors
/// * [`Error::InvalidSalePrice`] — `sale_price` ≤ 0.
/// * [`Error::RoyaltyOverflow`]  — `sale_price > i128::MAX / 10_000` or intermediate overflow.
pub fn safe_royalty_amount(sale_price: i128, basis_points: u32) -> Result<i128, Error> {
    if sale_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    // Pre-check: sale_price × 10_000 must fit in i128.
    if sale_price > i128::MAX / 10_000 {
        return Err(Error::RoyaltyOverflow);
    }
    let numerator = sale_price
        .checked_mul(basis_points as i128)
        .ok_or(Error::RoyaltyOverflow)?
        .checked_add(5_000)
        .ok_or(Error::RoyaltyOverflow)?;
    Ok(numerator / 10_000)
}
