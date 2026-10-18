//! Marketplace settlement logic (issues #1232, #1234, #1235, #1236).
//!
//! Single source of truth for distributing sale proceeds between
//! seller, creator (royalties) and platform.
//!
//! # Guarantees
//! - `seller_amount + royalty_amount + platform_fee == sale_price`
//! - Total deductions never exceed 100% (royalty bps + fee bps <= 10_000)
//! - Platform fee is derived from on-chain config (`platform_fee` module)
//! - NFT transfer + sold-marking only happen AFTER settlement succeeds

use soroban_sdk::{Address, Env};

use crate::platform_fee;
use crate::safe_math::safe_royalty_amount;
use crate::types::{DataKey, Error, TokenId};

/// Maximum combined deductions: 100% in basis points.
pub const MAX_TOTAL_DEDUCTIONS_BPS: u32 = 10_000;

/// Result of a marketplace settlement calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementDistribution {
    /// Amount owed to royalty recipients (creator).
    pub royalty_amount: i128,
    /// Amount owed to the platform.
    pub platform_fee: i128,
    /// Amount remaining for the seller.
    pub seller_amount: i128,
    /// Echo of the input sale price (for total-check).
    pub total: i128,
}

/// Retrieve the current platform fee in basis points.
pub fn get_platform_fee_bps(env: &Env) -> u32 {
    platform_fee::get_platform_fee(env)
}

/// Calculate the platform fee amount for a sale price.
///
/// # Errors
/// - [`Error::InvalidSalePrice`] if `sale_price <= 0`
/// - [`Error::InvalidBasisPoints`] if fee bps out of range (delegated)
/// - [`Error::RoyaltyOverflow`] on overflow
pub fn calculate_platform_fee_amount(sale_price: i128, fee_bps: u32) -> Result<i128, Error> {
    if sale_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    safe_royalty_amount(sale_price, fee_bps)
}

/// Calculate full settlement: royalty + platform fee + seller net.
///
/// Validates:
/// - `sale_price > 0`
/// - `royalty_bps + platform_fee_bps <= 10_000`
/// - `royalty_amount + platform_fee + seller_amount == sale_price`
///
/// # Errors
/// - [`Error::InvalidSalePrice`] on non-positive price or underflow
/// - [`Error::InvalidBasisPoints`] / [`Error::TotalDeductionsExceedSalePrice`]
///   when combined deductions exceed 100%
/// - [`Error::RoyaltyOverflow`] on overflow
pub fn calculate_settlement(
    sale_price: i128,
    royalty_bps: u32,
    platform_fee_bps: u32,
) -> Result<SettlementDistribution, Error> {
    if sale_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    let total_bps = royalty_bps
        .checked_add(platform_fee_bps)
        .ok_or(Error::RoyaltyOverflow)?;
    if total_bps > MAX_TOTAL_DEDUCTIONS_BPS {
        return Err(Error::TotalDeductionsExceedSalePrice);
    }

    let royalty_amount = safe_royalty_amount(sale_price, royalty_bps)?;
    let platform_fee = safe_royalty_amount(sale_price, platform_fee_bps)?;
    let seller_amount = sale_price
        .checked_sub(royalty_amount)
        .and_then(|v| v.checked_sub(platform_fee))
        .ok_or(Error::InvalidSalePrice)?;

    // Invariant: total distribution equals purchase amount (#1232).
    debug_assert_eq!(
        royalty_amount + platform_fee + seller_amount,
        sale_price
    );

    Ok(SettlementDistribution {
        royalty_amount,
        platform_fee,
        seller_amount,
        total: sale_price,
    })
}

/// Convenience: calculate settlement using the on-chain platform fee.
pub fn calculate_settlement_with_config(
    env: &Env,
    sale_price: i128,
    royalty_bps: u32,
) -> Result<SettlementDistribution, Error> {
    let fee_bps = get_platform_fee_bps(env);
    calculate_settlement(sale_price, royalty_bps, fee_bps)
}

/// Record of a completed sale (issue #1236).
#[soroban_sdk::contracttype]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoldListing {
    pub token_id: TokenId,
    pub seller: Address,
    pub buyer: Address,
    pub price: i128,
    pub sold_at: u64,
}

/// Persist a sold listing so future purchases are rejected (#1236).
///
/// Recommended: add `SoldListing(TokenId)` to `DataKey` in `types.rs`.
/// For compatibility this writes to the existing `DataKey::Listing(token_id)` slot.
pub fn mark_listing_sold(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    price: i128,
) -> Result<(), Error> {
    let sold = SoldListing {
        token_id,
        seller: seller.clone(),
        buyer: buyer.clone(),
        price,
        sold_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::Listing(token_id), &sold);
    Ok(())
}

/// Returns true if the token has already been sold.
pub fn is_listing_sold(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Listing(token_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settlement_total_equals_purchase_amount() {
        for price in [1_i128, 100, 999, 1_000_000, 123_456_789] {
            let s = calculate_settlement(price, 500, 250).unwrap();
            assert_eq!(s.royalty_amount + s.platform_fee + s.seller_amount, price);
            assert_eq!(s.total, price);
        }
    }

    #[test]
    fn platform_fee_included_in_settlement() {
        let s = calculate_settlement(10_000, 0, 1_000).unwrap();
        assert_eq!(s.platform_fee, 1_000);
        assert_eq!(s.seller_amount, 9_000);
    }

    #[test]
    fn rejects_deductions_over_100_percent() {
        assert_eq!(
            calculate_settlement(1_000, 9_000, 2_000),
            Err(Error::TotalDeductionsExceedSalePrice)
        );
    }

    #[test]
    fn rejects_non_positive_price() {
        assert_eq!(
            calculate_settlement(0, 500, 250),
            Err(Error::InvalidSalePrice)
        );
    }
}
