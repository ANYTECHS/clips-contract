//! Net seller amount calculator (issue #805).
//!
//! Given a sale price and a royalty rate, computes the royalty amount owed
//! to the creator and the amount that remains for the seller, guaranteeing
//! the two always sum back to the original sale price.

use crate::safe_math::safe_royalty_amount;
use crate::Error;

/// Result of a net seller amount calculation.
///
/// `royalty_amount + seller_amount` always equals the `sale_price` passed to
/// [`calculate_net_seller_amount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetSellerAmount {
    /// Amount owed to the royalty recipient, in the asset's smallest unit.
    pub royalty_amount: i128,
    /// Amount remaining for the seller, in the asset's smallest unit.
    pub seller_amount: i128,
}

/// Calculate the royalty amount and the remaining seller amount for a sale.
///
/// # Arguments
/// * `sale_price`   — Sale price in the asset's smallest unit. Must be > 0.
/// * `royalty_bps` — Royalty rate in basis points (1 bp = 0.01 %). Range: 0–10 000.
///
/// # Errors
/// * [`Error::InvalidSalePrice`] — `sale_price` ≤ 0.
/// * [`Error::RoyaltyOverflow`]  — arithmetic would overflow.
pub fn calculate_net_seller_amount(
    sale_price: i128,
    royalty_bps: u32,
) -> Result<NetSellerAmount, Error> {
    let royalty_amount = safe_royalty_amount(sale_price, royalty_bps)?;
    let seller_amount = sale_price - royalty_amount;
    Ok(NetSellerAmount {
        royalty_amount,
        seller_amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sale_price_between_royalty_and_seller() {
        let result = calculate_net_seller_amount(1_000_000, 500).unwrap();
        assert_eq!(result.royalty_amount, 50_000);
        assert_eq!(result.seller_amount, 950_000);
    }

    #[test]
    fn royalty_and_seller_amount_always_sum_to_sale_price() {
        for sale_price in [1_i128, 7, 100, 999, 1_000_000, 123_456_789] {
            for bps in [0_u32, 1, 250, 500, 2_500, 5_000, 9_999, 10_000] {
                let result = calculate_net_seller_amount(sale_price, bps).unwrap();
                assert_eq!(result.royalty_amount + result.seller_amount, sale_price);
            }
        }
    }

    #[test]
    fn zero_bps_gives_entire_amount_to_seller() {
        let result = calculate_net_seller_amount(1_000_000, 0).unwrap();
        assert_eq!(result.royalty_amount, 0);
        assert_eq!(result.seller_amount, 1_000_000);
    }

    #[test]
    fn max_bps_gives_entire_amount_to_royalty() {
        let result = calculate_net_seller_amount(1_000_000, 10_000).unwrap();
        assert_eq!(result.royalty_amount, 1_000_000);
        assert_eq!(result.seller_amount, 0);
    }

    #[test]
    fn rejects_non_positive_sale_price() {
        assert_eq!(
            calculate_net_seller_amount(0, 500),
            Err(Error::InvalidSalePrice)
        );
        assert_eq!(
            calculate_net_seller_amount(-1, 500),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn rejects_overflowing_sale_price() {
        assert_eq!(
            calculate_net_seller_amount(i128::MAX, 500),
            Err(Error::RoyaltyOverflow)
        );
    }
}
