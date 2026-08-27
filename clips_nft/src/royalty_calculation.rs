//! Basis point calculation (issue #800).
//!
//! Precise royalty percentages are computed using **basis points** (bps):
//! 1 bp = 0.01 %, and 10 000 bps = 100 %.
//!
//! # Denominander
//! [`BPS_DENOMINATOR`] is the single source of truth for the basis-point
//! scale used across every percentage conversion in this module.
//!
//! # Guarantees
//! - Percentages are always derived from the shared denominator.
//! - Zero royalty (`bps == 0` or all recipients at `0` bps) is detected and
//!   short-circuited instead of being routed through payment plumbing.
//! - All arithmetic is overflow-checked and returns [`Error::RoyaltyOverflow`].

use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::{Error, Royalty};

/// Denominator for basis-point calculations: 10 000 bps represent 100 %.
///
/// Mirrors [`crate::storage_constants::BASIS_POINTS_SCALE`] as the canonical
/// constant for every percentage computation performed here.
pub const BPS_DENOMINATOR: u32 = 10_000;

/// Compute the percentage of `amount` represented by `basis_points`.
///
/// ```text
/// result = round_half_up(amount × basis_points / BPS_DENOMINATOR)
/// ```
///
/// Rounding is deterministic round-half-up, consistent with the contract's
/// royalty math; the numerator is incremented by half the denominator before
/// the final division so that ex. `1 × 500 / 10_000 = 0.05` rounds to `0`
/// while `1 × 5_000 / 10_000 = 0.5` rounds up to `1`.
///
/// # Arguments
/// * `amount` — value in the asset's smallest unit; must be `>= 0`.
/// * `basis_points` — royalty rate in bps; must be `0..=MAX_ROYALTY_BPS`.
///
/// # Errors
/// * [`Error::InvalidBasisPoints`] — `basis_points > MAX_ROYALTY_BPS`.
/// * [`Error::InvalidSalePrice`]   — `amount` is negative.
/// * [`Error::RoyaltyOverflow`]    — `amount × basis_points` overflows `i128`.
pub fn basis_point_percentage(amount: i128, basis_points: u32) -> Result<i128, Error> {
    if basis_points > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if basis_points == 0 {
        return Ok(0);
    }
    if amount < 0 {
        return Err(Error::InvalidSalePrice);
    }
    if amount == 0 {
        return Ok(0);
    }
    let numerator = amount
        .checked_mul(basis_points as i128)
        .ok_or(Error::RoyaltyOverflow)?;
    let rounded = numerator
        .checked_add(BPS_DENOMINATOR as i128 / 2)
        .ok_or(Error::RoyaltyOverflow)?;
    Ok(rounded / BPS_DENOMINATOR as i128)
}

/// Return `true` when a royalty configuration carries no payable share.
///
/// A royalty is "zero" when it has no recipients or when the total basis
/// points across all recipients is zero. Zero-royalty configurations must be
/// short-circuited so no unnecessary payment operation is attempted.
pub fn is_zero_royalty(royalty: &Royalty) -> bool {
    if royalty.recipients.is_empty() {
        return true;
    }
    let mut total_bps: u32 = 0;
    for i in 0..royalty.recipients.len() {
        if let Some(r) = royalty.recipients.get(i) {
            total_bps = total_bps.saturating_add(r.basis_points);
        }
    }
    total_bps == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyRecipient;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn zero_royalty(env: &Env) -> Royalty {
        Royalty {
            recipients: soroban_sdk::vec![env, RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 0,
            }],
            asset_address: None,
        }
    }

    fn royalty(env: &Env, bps: u32) -> Royalty {
        Royalty {
            recipients: soroban_sdk::vec![env, RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: bps,
            }],
            asset_address: None,
        }
    }

    // ── basis_point_percentage ──────────────────────────────────────────────

    #[test]
    fn zero_bps_returns_zero() {
        assert_eq!(basis_point_percentage(1_000_000, 0).unwrap(), 0);
        assert_eq!(basis_point_percentage(0, 500).unwrap(), 0);
    }

    #[test]
    fn one_percent_is_100_bps() {
        assert_eq!(basis_point_percentage(1_000_000, 100).unwrap(), 10_000);
    }

    #[test]
    fn five_percent_is_500_bps() {
        assert_eq!(basis_point_percentage(1_000_000, 500).unwrap(), 50_000);
    }

    #[test]
    fn full_royalty_is_10_000_bps() {
        assert_eq!(basis_point_percentage(1_000_000, 10_000).unwrap(), 1_000_000);
    }

    #[test]
    fn boundary_max_bps_accepted() {
        assert!(basis_point_percentage(100, 10_000).is_ok());
    }

    #[test]
    fn boundary_above_max_bps_rejected() {
        assert_eq!(
            basis_point_percentage(100, 10_001),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn boundary_u32_max_bps_rejected() {
        assert_eq!(
            basis_point_percentage(100, u32::MAX),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn halves_round_up() {
        // 1 × 5_000 / 10_000 = 0.5 → 1 (round-half-up)
        assert_eq!(basis_point_percentage(1, 5_000).unwrap(), 1);
    }

    #[test]
    fn sub_unit_amount_rounds_to_zero() {
        // 1 × 100 / 10_000 = 0.01 → 0
        assert_eq!(basis_point_percentage(1, 100).unwrap(), 0);
    }

    #[test]
    fn negative_amount_rejected() {
        assert_eq!(
            basis_point_percentage(-5, 500),
            Err(Error::InvalidSalePrice)
        );
    }

    #[test]
    fn overflow_detected() {
        let huge = i128::MAX;
        assert_eq!(
            basis_point_percentage(huge, 10_000),
            Err(Error::RoyaltyOverflow)
        );
    }

    // ── is_zero_royalty ─────────────────────────────────────────────────────

    #[test]
    fn empty_recipients_is_zero_royalty() {
        let env = Env::default();
        let royalty = Royalty {
            recipients: soroban_sdk::Vec::new(&env),
            asset_address: None,
        };
        assert!(is_zero_royalty(&royalty));
    }

    #[test]
    fn all_zero_bps_is_zero_royalty() {
        let env = Env::default();
        assert!(is_zero_royalty(&zero_royalty(&env)));
    }

    #[test]
    fn any_positive_bps_is_not_zero() {
        let env = Env::default();
        assert!(!is_zero_royalty(&royalty(&env, 1)));
        assert!(!is_zero_royalty(&royalty(&env, 10_000)));
    }

    #[test]
    fn multi_recipient_zero_sum_is_zero() {
        let env = Env::default();
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient { recipient: Address::generate(&env), basis_points: 0 },
                RoyaltyRecipient { recipient: Address::generate(&env), basis_points: 0 },
            ],
            asset_address: None,
        };
        assert!(is_zero_royalty(&royalty));
    }
}