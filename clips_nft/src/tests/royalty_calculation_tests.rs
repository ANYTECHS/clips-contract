//! Royalty calculation test suite — resolves issue #808.
//!
//! Comprehensive tests for royalty amount calculations, basis-point
//! conversions, overflow safety, rounding behaviour, and combined
//! deduction validation.

#![cfg(test)]

use crate::safe_math::{safe_royalty_amount, ASSET_SCALE};
use crate::storage_constants::{MAX_PLATFORM_FEE_BPS, MAX_ROYALTY_BPS};
use crate::transaction_deduction_validator::{
    validate_config_deductions, validate_total_deduction_amount, validate_total_deduction_bps,
    MAX_TOTAL_DEDUCTION_BPS,
};

// ─── safe_royalty_amount ─────────────────────────────────────────────────────

#[test]
fn royalty_zero_bps_returns_zero() {
    assert_eq!(safe_royalty_amount(1_000_000, 0).unwrap(), 0);
}

#[test]
fn royalty_one_bp_on_large_price() {
    // 1 bp = 0.01 %;  1_000_000 × 1 / 10_000 = 100
    assert_eq!(safe_royalty_amount(1_000_000, 1).unwrap(), 100);
}

#[test]
fn royalty_ten_percent() {
    // 1_000_bps = 10 %;  1_000_000 × 1_000 / 10_000 = 100_000
    assert_eq!(safe_royalty_amount(1_000_000, 1_000).unwrap(), 100_000);
}

#[test]
fn royalty_fifty_percent() {
    assert_eq!(safe_royalty_amount(1_000_000, 5_000).unwrap(), 500_000);
}

#[test]
fn royalty_full_100_percent() {
    assert_eq!(safe_royalty_amount(1_000_000, 10_000).unwrap(), 1_000_000);
}

#[test]
fn royalty_small_price_high_bps() {
    // 10 × 5_000 / 10_000 = 5
    assert_eq!(safe_royalty_amount(10, 5_000).unwrap(), 5);
}

#[test]
fn royalty_fractional_rounds_correctly() {
    // 15 × 3333 × ASSET_SCALE + 5_000 = 499_950_000_005_000
    // / 10_000 / ASSET_SCALE = 4
    let result = safe_royalty_amount(15, 3_333).unwrap();
    assert_eq!(result, 4);
}

#[test]
fn royalty_fractional_rounds_down() {
    // 10 × 3333 / 10_000 = 3.333 → rounds to 3
    let result = safe_royalty_amount(10, 3_333).unwrap();
    assert_eq!(result, 3);
}

#[test]
fn royalty_preserves_subunit_precision() {
    // With ASSET_SCALE = 10_000_000, small fractional amounts should not vanish.
    // 7 × 1_500 / 10_000 = 1.05 → 1
    let result = safe_royalty_amount(7, 1_500).unwrap();
    assert_eq!(result, 1);
}

#[test]
fn royalty_no_overflow_on_max_safe_price() {
    // Well below overflow threshold
    let price = 1_000_000_000_000i128; // 1 trillion
    let result = safe_royalty_amount(price, 2_500);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 250_000_000_000);
}

#[test]
fn royalty_overflow_on_extreme_price() {
    // price > i128::MAX / (10_000 × ASSET_SCALE) → overflow
    let extreme = i128::MAX / (10_000 * ASSET_SCALE) + 1;
    assert_eq!(
        safe_royalty_amount(extreme, 1),
        Err(crate::Error::RoyaltyOverflow)
    );
}

#[test]
fn royalty_invalid_zero_price() {
    assert_eq!(
        safe_royalty_amount(0, 500),
        Err(crate::Error::InvalidSalePrice)
    );
}

#[test]
fn royalty_invalid_negative_price() {
    assert_eq!(
        safe_royalty_amount(-100, 500),
        Err(crate::Error::InvalidSalePrice)
    );
}

#[test]
fn royalty_1_bp_on_1_stroop() {
    // Minimum price: 1 stroop; 1 × 1 / 10_000 = 0 (rounds to 0)
    assert_eq!(safe_royalty_amount(1, 1).unwrap(), 0);
}

#[test]
fn royalty_asymmetry_high_bps_low_price() {
    // 1 stroop at 10_000 bps = 100%
    assert_eq!(safe_royalty_amount(1, 10_000).unwrap(), 1);
}

// ─── validate_total_deduction_bps ────────────────────────────────────────────

#[test]
fn combined_bps_valid_under_limit() {
    assert!(validate_total_deduction_bps(500, 100).is_ok());
}

#[test]
fn combined_bps_exactly_at_limit() {
    assert!(validate_total_deduction_bps(10_000, 0).is_ok());
    assert!(validate_total_deduction_bps(0, 10_000).is_ok());
    assert!(validate_total_deduction_bps(9_000, 1_000).is_ok());
}

#[test]
fn combined_bps_exceeds_limit() {
    assert_eq!(
        validate_total_deduction_bps(9_500, 1_000),
        Err(crate::Error::TotalDeductionsExceedSalePrice)
    );
}

#[test]
fn combined_bps_overflow_protection() {
    assert_eq!(
        validate_total_deduction_bps(u32::MAX, 1),
        Err(crate::Error::TotalDeductionsExceedSalePrice)
    );
}

#[test]
fn combined_bps_both_at_max_individual() {
    // MAX_ROYALTY_BPS=10_000, MAX_PLATFORM_FEE_BPS=1_000 → total=11_000 > 10_000
    assert_eq!(
        validate_total_deduction_bps(MAX_ROYALTY_BPS, MAX_PLATFORM_FEE_BPS),
        Err(crate::Error::TotalDeductionsExceedSalePrice)
    );
}

// ─── validate_total_deduction_amount ─────────────────────────────────────────

#[test]
fn amount_validation_valid_combination() {
    let result = validate_total_deduction_amount(1_000_000, 500, 100);
    assert!(result.is_ok());
    let (royalty, fee) = result.unwrap();
    assert_eq!(royalty, 50_000);
    assert_eq!(fee, 10_000);
    assert!(royalty + fee <= 1_000_000);
}

#[test]
fn amount_validation_zero_price() {
    assert_eq!(
        validate_total_deduction_amount(0, 500, 100),
        Err(crate::Error::InvalidSalePrice)
    );
}

#[test]
fn amount_validation_negative_price() {
    assert_eq!(
        validate_total_deduction_amount(-500, 500, 100),
        Err(crate::Error::InvalidSalePrice)
    );
}

#[test]
fn amount_validation_exceeds_price() {
    // 9_500 + 1_000 = 10_500 bps > 10_000 → deductions > sale_price
    assert_eq!(
        validate_total_deduction_amount(10_000, 9_500, 1_000),
        Err(crate::Error::TotalDeductionsExceedSalePrice)
    );
}

#[test]
fn amount_validation_exactly_100_percent() {
    let result = validate_total_deduction_amount(10_000, 10_000, 0);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().0, 10_000);
}

#[test]
fn amount_validation_small_price_large_bps() {
    // 1 stroop at 10_000 bps = 1 stroop royalty, no fee
    let result = validate_total_deduction_amount(1, 10_000, 0);
    assert!(result.is_ok());
}

// ─── validate_config_deductions ──────────────────────────────────────────────

#[test]
fn config_deductions_valid() {
    assert!(validate_config_deductions(500, 100).is_ok());
}

#[test]
fn config_deductions_royalty_too_high() {
    assert_eq!(
        validate_config_deductions(10_001, 100),
        Err(crate::Error::InvalidBasisPoints)
    );
}

#[test]
fn config_deductions_fee_too_high() {
    assert_eq!(
        validate_config_deductions(500, 1_001),
        Err(crate::Error::InvalidBasisPoints)
    );
}

#[test]
fn config_deductions_both_too_high() {
    assert_eq!(
        validate_config_deductions(10_001, 1_001),
        Err(crate::Error::InvalidBasisPoints)
    );
}

#[test]
fn config_deductions_combined_exceeds_100_percent() {
    // Both within individual limits but combined > 100%
    assert_eq!(
        validate_config_deductions(9_500, 1_000),
        Err(crate::Error::TotalDeductionsExceedSalePrice)
    );
}

#[test]
fn config_deductions_boundary_zero() {
    assert!(validate_config_deductions(0, 0).is_ok());
}

#[test]
fn config_deductions_boundary_max_royalty_only() {
    assert!(validate_config_deductions(MAX_ROYALTY_BPS, 0).is_ok());
}

#[test]
fn config_deductions_boundary_max_fee_only() {
    assert!(validate_config_deductions(0, MAX_PLATFORM_FEE_BPS).is_ok());
}

// ─── Real-world scenarios ────────────────────────────────────────────────────

#[test]
fn scenario_standard_sale_5_percent_royalty_2_percent_fee() {
    let sale_price = 100_000_000; // 10 XLM
    let royalty_bps = 500; // 5%
    let fee_bps = 200; // 2%

    let (royalty, fee) = validate_total_deduction_amount(sale_price, royalty_bps, fee_bps).unwrap();
    assert_eq!(royalty, 5_000_000); // 0.5 XLM
    assert_eq!(fee, 2_000_000); // 0.2 XLM
    assert!(royalty + fee < sale_price);
}

#[test]
fn scenario_high_royalty_creator_fairness() {
    let sale_price = 50_000_000; // 5 XLM
    let royalty_bps = 1_500; // 15%
    let fee_bps = 100; // 1%

    let (royalty, fee) = validate_total_deduction_amount(sale_price, royalty_bps, fee_bps).unwrap();
    assert_eq!(royalty, 7_500_000);
    assert_eq!(fee, 500_000);
    assert!(royalty + fee <= sale_price);
}

#[test]
fn scenario_micro_sale() {
    let sale_price = 100; // 0.00001 XLM
    let royalty_bps = 500;
    let fee_bps = 200;

    let (royalty, fee) = validate_total_deduction_amount(sale_price, royalty_bps, fee_bps).unwrap();
    // 100 × 500 / 10_000 = 5
    assert_eq!(royalty, 5);
    // 100 × 200 / 10_000 = 2
    assert_eq!(fee, 2);
    assert!(royalty + fee <= sale_price);
}

#[test]
fn scenario_max_individual_but_safe_combined() {
    // 5000 bps royalty + 1000 bps fee = 6000 bps total = 60%
    let sale_price = 10_000_000;
    let (royalty, fee) = validate_total_deduction_amount(sale_price, 5_000, 1_000).unwrap();
    assert_eq!(royalty, 5_000_000);
    assert_eq!(fee, 1_000_000);
    assert!(royalty + fee <= sale_price);
}

#[test]
fn scenario_no_royalty_no_fee() {
    let sale_price = 1_000_000;
    let (royalty, fee) = validate_total_deduction_amount(sale_price, 0, 0).unwrap();
    assert_eq!(royalty, 0);
    assert_eq!(fee, 0);
}

#[test]
fn scenario_royalty_only_no_fee() {
    let sale_price = 1_000_000;
    let (royalty, fee) = validate_total_deduction_amount(sale_price, 1_000, 0).unwrap();
    assert_eq!(royalty, 100_000);
    assert_eq!(fee, 0);
}

#[test]
fn scenario_fee_only_no_royalty() {
    let sale_price = 1_000_000;
    let (royalty, fee) = validate_total_deduction_amount(sale_price, 0, 1_000).unwrap();
    assert_eq!(royalty, 0);
    assert_eq!(fee, 100_000);
}
