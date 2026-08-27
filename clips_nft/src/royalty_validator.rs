//! Royalty validator — validates royalty configuration before minting or updating.
//!
//! Resolves issue #430: validate royalty percentages.

use crate::types::{Error, Royalty};

/// Maximum allowed royalty in basis points (100 % = 10 000 bps).
pub const MAX_ROYALTY_BPS: u32 = 10_000;

/// Validate a `Royalty` struct.
///
/// Checks:
/// - Total `basis_points` across all recipients does not exceed [`MAX_ROYALTY_BPS`].
/// - Each individual recipient's `basis_points` does not exceed [`MAX_ROYALTY_BPS`].
/// - At least one recipient is present.
///
/// Returns `Err(Error::InvalidBasisPoints)` when validation fails.
pub fn validate_royalty(royalty: &Royalty) -> Result<(), Error> {
    if royalty.recipients.is_empty() {
        return Err(Error::InvalidBasisPoints);
    }
    let mut total_bps: u32 = 0;
    for r in royalty.recipients.iter() {
        if r.basis_points > MAX_ROYALTY_BPS {
            return Err(Error::InvalidBasisPoints);
        }
        total_bps = total_bps.saturating_add(r.basis_points);
    }
    if total_bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    Ok(())
}

/// Validate a raw basis-points value without a full `Royalty` struct.
///
/// Returns `Err(Error::InvalidBasisPoints)` when `bps > MAX_ROYALTY_BPS`.
pub fn validate_royalty_bps(bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyRecipient;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_royalty(env: &Env, bps: u32) -> Royalty {
        Royalty {
            recipients: soroban_sdk::vec![env, RoyaltyRecipient { recipient: Address::generate(env), basis_points: bps }],
            asset_address: None,
        }
    }

    #[test]
    fn valid_royalty_passes() {
        let env = Env::default();
        assert!(validate_royalty(&make_royalty(&env, 500)).is_ok());
        assert!(validate_royalty(&make_royalty(&env, 0)).is_ok());
        assert!(validate_royalty(&make_royalty(&env, 10_000)).is_ok());
    }

    #[test]
    fn royalty_too_high_fails() {
        let env = Env::default();
        assert_eq!(
            validate_royalty(&make_royalty(&env, 10_001)),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn validate_bps_boundary() {
        assert!(validate_royalty_bps(10_000).is_ok());
        assert_eq!(validate_royalty_bps(10_001), Err(Error::InvalidBasisPoints));
    }
}
