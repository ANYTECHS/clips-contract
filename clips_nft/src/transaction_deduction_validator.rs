//! Transaction deduction validator — ensures total deductions don't exceed sale price.
//!
//! Resolves issue #807: Validate total transaction deductions.
//!
//! When processing a sale or transfer with royalties, the combined total of:
//! - Royalty amount (based on royalty basis points)
//! - Platform fee (based on platform fee basis points)
//!
//! Must not exceed 100% of the sale price. This module provides validation
//! functions to ensure this invariant is maintained.

use crate::types::Error;

/// Maximum total deductions in basis points (100% = 10,000).
pub const MAX_TOTAL_DEDUCTION_BPS: u32 = 10_000;

/// Validate that the combined royalty and platform fee basis points don't exceed 100%.
///
/// # Arguments
/// * `royalty_bps` - Royalty percentage in basis points (0–10,000)
/// * `platform_fee_bps` - Platform fee percentage in basis points (0–1,000)
///
/// # Returns
/// - `Ok(())` if the combined total is valid
/// - `Err(Error::TotalDeductionsExceedSalePrice)` if the combined total exceeds 100%
pub fn validate_total_deduction_bps(royalty_bps: u32, platform_fee_bps: u32) -> Result<(), Error> {
    let total_bps = royalty_bps
        .checked_add(platform_fee_bps)
        .ok_or(Error::TotalDeductionsExceedSalePrice)?;

    if total_bps > MAX_TOTAL_DEDUCTION_BPS {
        return Err(Error::TotalDeductionsExceedSalePrice);
    }

    Ok(())
}

/// Validate that the combined royalty and platform fee amounts don't exceed the sale price.
///
/// This function calculates the actual amounts and verifies the total deduction
/// is less than or equal to the sale price.
///
/// # Arguments
/// * `sale_price` - Sale price in the asset's smallest unit (must be > 0)
/// * `royalty_bps` - Royalty percentage in basis points (0–10,000)
/// * `platform_fee_bps` - Platform fee percentage in basis points (0–1,000)
///
/// # Returns
/// - `Ok((royalty_amount, platform_fee_amount))` - The calculated amounts
/// - `Err(Error::InvalidSalePrice)` - If sale_price <= 0
/// - `Err(Error::RoyaltyOverflow)` - If calculation would overflow
/// - `Err(Error::TotalDeductionsExceedSalePrice)` - If total deductions exceed sale price
pub fn validate_total_deduction_amount(
    sale_price: i128,
    royalty_bps: u32,
    platform_fee_bps: u32,
) -> Result<(i128, i128), Error> {
    if sale_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }

    // Calculate royalty amount using safe math
    let royalty_amount = crate::safe_math::safe_royalty_amount(sale_price, royalty_bps)?;

    // Calculate platform fee amount using safe math
    let platform_fee_amount = crate::safe_math::safe_royalty_amount(sale_price, platform_fee_bps)?;

    // Check that total deductions don't exceed sale price
    let total_deductions = royalty_amount
        .checked_add(platform_fee_amount)
        .ok_or(Error::RoyaltyOverflow)?;

    if total_deductions > sale_price {
        return Err(Error::TotalDeductionsExceedSalePrice);
    }

    Ok((royalty_amount, platform_fee_amount))
}

/// Validate a configuration struct's royalty and platform fee combined total.
///
/// This is a convenience function that validates both the individual ranges
/// and the combined total for configuration updates.
///
/// # Arguments
/// * `royalty_bps` - Default royalty percentage in basis points
/// * `platform_fee_bps` - Platform fee percentage in basis points
///
/// # Returns
/// - `Ok(())` if the configuration is valid
/// - `Err(Error::InvalidBasisPoints)` - If either value exceeds its individual maximum
/// - `Err(Error::TotalDeductionsExceedSalePrice)` - If the combined total exceeds 100%
pub fn validate_config_deductions(royalty_bps: u32, platform_fee_bps: u32) -> Result<(), Error> {
    // Validate individual ranges
    if royalty_bps > crate::storage_constants::MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if platform_fee_bps > crate::storage_constants::MAX_PLATFORM_FEE_BPS {
        return Err(Error::InvalidBasisPoints);
    }

    // Validate combined total
    validate_total_deduction_bps(royalty_bps, platform_fee_bps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_total_deduction_bps_valid() {
        assert!(validate_total_deduction_bps(500, 100).is_ok());
    }

    #[test]
    fn test_validate_total_deduction_bps_boundary() {
        assert!(validate_total_deduction_bps(10_000, 0).is_ok());
    }

    #[test]
    fn test_validate_total_deduction_bps_exceeds() {
        assert_eq!(
            validate_total_deduction_bps(9_500, 1_000),
            Err(Error::TotalDeductionsExceedSalePrice)
        );
    }

    #[test]
    fn test_validate_total_deduction_bps_overflow() {
        assert_eq!(
            validate_total_deduction_bps(u32::MAX, 1),
            Err(Error::TotalDeductionsExceedSalePrice)
        );
    }

    #[test]
    fn test_validate_total_deduction_amount_valid() {
        let sale_price = 1_000_000;
        let royalty_bps = 500;
        let platform_fee_bps = 100;

        let result = validate_total_deduction_amount(sale_price, royalty_bps, platform_fee_bps);
        assert!(result.is_ok());

        let (royalty_amount, platform_fee_amount) = result.unwrap();
        assert_eq!(royalty_amount, 50_000);
        assert_eq!(platform_fee_amount, 10_000);
        assert!(royalty_amount + platform_fee_amount <= sale_price);
    }

    #[test]
    fn test_validate_total_deduction_amount_invalid_price() {
        let result = validate_total_deduction_amount(0, 500, 100);
        assert_eq!(result, Err(Error::InvalidSalePrice));

        let result = validate_total_deduction_amount(-100, 500, 100);
        assert_eq!(result, Err(Error::InvalidSalePrice));
    }

    #[test]
    fn test_validate_total_deduction_amount_boundary() {
        let sale_price = 10_000;
        let royalty_bps = 10_000;
        let platform_fee_bps = 0;

        let result = validate_total_deduction_amount(sale_price, royalty_bps, platform_fee_bps);
        assert!(result.is_ok());

        let (royalty_amount, platform_fee_amount) = result.unwrap();
        assert_eq!(royalty_amount, 10_000);
        assert_eq!(platform_fee_amount, 0);
        assert!(royalty_amount + platform_fee_amount <= sale_price);
    }

    #[test]
    fn test_validate_total_deduction_amount_exceeds_price() {
        let sale_price = 10_000;
        let royalty_bps = 9_500;
        let platform_fee_bps = 1_000;

        let result = validate_total_deduction_amount(sale_price, royalty_bps, platform_fee_bps);
        assert_eq!(result, Err(Error::TotalDeductionsExceedSalePrice));
    }

    #[test]
    fn test_validate_config_deductions_valid() {
        assert!(validate_config_deductions(500, 100).is_ok());
    }

    #[test]
    fn test_validate_config_deductions_invalid_royalty() {
        assert_eq!(
            validate_config_deductions(10_001, 100),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn test_validate_config_deductions_invalid_platform_fee() {
        assert_eq!(
            validate_config_deductions(500, 1_001),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn test_validate_config_deductions_exceeds_total() {
        assert_eq!(
            validate_config_deductions(9_500, 1_000),
            Err(Error::TotalDeductionsExceedSalePrice)
        );
    }
}
