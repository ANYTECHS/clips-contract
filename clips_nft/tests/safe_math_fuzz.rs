//! Property-based fuzz tests for [`clips_nft::safe_math::safe_royalty_amount`].

use clips_nft::{safe_math, Error};
use proptest::prelude::*;

fn reference_royalty(sale_price: i128, basis_points: u32) -> Option<i128> {
    if sale_price <= 0 {
        return None;
    }
    if sale_price > i128::MAX / 10_000 {
        return None;
    }
    let numerator = sale_price
        .checked_mul(basis_points as i128)?
        .checked_add(5_000)?;
    Some(numerator / 10_000)
}

proptest! {
    #[test]
    fn safe_royalty_matches_reference_for_valid_inputs(
        sale_price in 1i128..=(i128::MAX / 10_000),
        basis_points in 0u32..=10_000u32,
    ) {
        let expected = reference_royalty(sale_price, basis_points).unwrap();
        let actual = safe_math::safe_royalty_amount(sale_price, basis_points).unwrap();
        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn safe_royalty_rejects_non_positive_sale_price(sale_price in i128::MIN..=0i128) {
        prop_assert_eq!(
            safe_math::safe_royalty_amount(sale_price, 500),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn safe_royalty_rejects_overflowing_sale_price(
        sale_price in (i128::MAX / 10_000 + 1)..=i128::MAX,
        basis_points in 0u32..=10_000u32,
    ) {
        prop_assert_eq!(
            safe_math::safe_royalty_amount(sale_price, basis_points),
            Err(Error::RoyaltyOverflow)
        );
    }

    #[test]
    fn safe_royalty_rejects_basis_points_overflow_at_extreme_price(
        sale_price in 1i128..=(i128::MAX / 10_000),
        basis_points in 10_001u32..=u32::MAX,
    ) {
        // basis_points as i128 mul can overflow when very large; contract casts to i128.
        let result = safe_math::safe_royalty_amount(sale_price, basis_points);
        if sale_price.checked_mul(basis_points as i128).is_none() {
            prop_assert_eq!(result, Err(Error::RoyaltyOverflow));
        }
    }

    #[test]
    fn safe_royalty_amount_never_exceeds_sale_price(
        sale_price in 1i128..=(i128::MAX / 10_000),
        basis_points in 0u32..=10_000u32,
    ) {
        let amount = safe_math::safe_royalty_amount(sale_price, basis_points).unwrap();
        prop_assert!(amount <= sale_price);
    }
}

#[test]
fn safe_royalty_boundary_max_safe_price() {
    let sale_price = i128::MAX / 10_000;
    let amount = safe_math::safe_royalty_amount(sale_price, 10_000).unwrap();
    assert!(amount > 0);
}

#[test]
fn safe_royalty_boundary_one_stroop_sale() {
    assert_eq!(safe_math::safe_royalty_amount(1, 10_000).unwrap(), 1);
}

#[test]
fn safe_royalty_edge_case_very_small_price() {
    // Test with minimum valid price (1 stroop)
    assert_eq!(safe_math::safe_royalty_amount(1, 0).unwrap(), 0);
    assert_eq!(safe_math::safe_royalty_amount(1, 1).unwrap(), 0); // (1*1+5000)/10000 = 0
    assert_eq!(safe_math::safe_royalty_amount(1, 10_000).unwrap(), 1);
}

#[test]
fn safe_royalty_edge_case_very_large_basis_points() {
    // Test with basis points at u32::MAX (should overflow)
    let result = safe_math::safe_royalty_amount(1_000, u32::MAX);
    assert_eq!(result, Err(Error::RoyaltyOverflow));
}

#[test]
fn safe_royalty_edge_case_boundary_exact_division() {
    // Test where (sale_price * basis_points) is exactly divisible by 10_000
    let sale_price = 10_000;
    let basis_points = 5_000; // 50%
    let result = safe_math::safe_royalty_amount(sale_price, basis_points).unwrap();
    // (10_000 * 5_000 + 5_000) / 10_000 = 50_005_000 / 10_000 = 5_000
    assert_eq!(result, 5_000);
}

#[test]
fn safe_royalty_edge_case_rounding_up() {
    // Test rounding behavior with values that round up
    let sale_price = 9_999;
    let basis_points = 5_000; // 50%
    let result = safe_math::safe_royalty_amount(sale_price, basis_points).unwrap();
    // (9_999 * 5_000 + 5_000) / 10_000 = 49_995_000 + 5_000 / 10_000 = 50_000 / 10_000 = 5
    assert_eq!(result, 5);
}

#[test]
fn safe_royalty_edge_case_accumulation_safety() {
    // Test that repeated calculations don't cause issues
    let price = 1_000_000_000;
    for bps in [0, 100, 1000, 5000, 10000] {
        let result = safe_math::safe_royalty_amount(price, bps);
        assert!(result.is_ok());
    }
}

#[test]
fn safe_royalty_edge_case_max_basis_points_various_prices() {
    // Test 100% royalty (10,000 basis points) across various price ranges
    let prices = [1, 100, 10_000, 1_000_000, i128::MAX / 10_000];
    for price in prices {
        let result = safe_math::safe_royalty_amount(price, 10_000);
        assert!(result.is_ok());
        // 100% royalty should equal the price (with rounding)
        let royalty = result.unwrap();
        assert!(royalty <= price || price == i128::MAX / 10_000);
    }
}
