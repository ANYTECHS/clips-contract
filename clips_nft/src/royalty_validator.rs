//! Royalty validator — validates royalty configuration before minting or updating.
//!
//! Resolves issue #430: validate royalty percentages.

use crate::types::{Error, Royalty};

/// Maximum allowed royalty in basis points (100 % = 10 000 bps).
pub const MAX_ROYALTY_BPS: u32 = 10_000;

/// Standardized errors for invalid royalty configurations.
///
/// Resolves issue #1401: create invalid royalty error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoyaltyValidationError {
    /// Royalty basis points exceed [`MAX_ROYALTY_BPS`].
    InvalidRoyaltyBps,
    /// A royalty recipient is missing or invalid.
    InvalidRoyaltyRecipient,
    /// The royalty state is inconsistent or otherwise invalid.
    InvalidRoyaltyState,
    /// The caller is not authorized to update the royalty configuration.
    UnauthorizedRoyaltyUpdate,
}

impl RoyaltyValidationError {
    /// Map a royalty validation error into the contract-level [`Error`].
    pub fn to_error(self) -> Error {
        match self {
            RoyaltyValidationError::InvalidRoyaltyBps => Error::InvalidBasisPoints,
            RoyaltyValidationError::InvalidRoyaltyRecipient => Error::InvalidBasisPoints,
            RoyaltyValidationError::InvalidRoyaltyState => Error::InvalidBasisPoints,
            RoyaltyValidationError::UnauthorizedRoyaltyUpdate => Error::Unauthorized,
        }
    }
}

/// Validate a `Royalty` struct.
///
/// Checks:
/// - Total `basis_points` across all recipients does not exceed [`MAX_ROYALTY_BPS`].
/// - Each individual recipient's `basis_points` does not exceed [`MAX_ROYALTY_BPS`].
/// - At least one recipient is present.
///
/// Returns a [`RoyaltyValidationError`] describing the first failure.
pub fn validate_royalty_config(royalty: &Royalty) -> Result<(), RoyaltyValidationError> {
    if royalty.recipients.is_empty() {
        return Err(RoyaltyValidationError::InvalidRoyaltyRecipient);
    }
    let mut total_bps: u32 = 0;
    for r in royalty.recipients.iter() {
        if r.basis_points > MAX_ROYALTY_BPS {
            return Err(RoyaltyValidationError::InvalidRoyaltyBps);
        }
        total_bps = total_bps.saturating_add(r.basis_points);
    }
    if total_bps > MAX_ROYALTY_BPS {
        return Err(RoyaltyValidationError::InvalidRoyaltyBps);
    }
    Ok(())
}

/// Validate a `Royalty` struct, mapping failures into the contract-level [`Error`].
pub fn validate_royalty(royalty: &Royalty) -> Result<(), Error> {
    validate_royalty_config(royalty).map_err(RoyaltyValidationError::to_error)
}

/// Validate a raw basis-points value without a full `Royalty` struct.
///
/// Returns [`RoyaltyValidationError::InvalidRoyaltyBps`] when `bps > MAX_ROYALTY_BPS`.
pub fn validate_royalty_bps_config(bps: u32) -> Result<(), RoyaltyValidationError> {
    if bps > MAX_ROYALTY_BPS {
        return Err(RoyaltyValidationError::InvalidRoyaltyBps);
    }
    Ok(())
}

/// Validate a raw basis-points value, mapping failures into the contract-level [`Error`].
pub fn validate_royalty_bps(bps: u32) -> Result<(), Error> {
    validate_royalty_bps_config(bps).map_err(RoyaltyValidationError::to_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyRecipient;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn make_royalty(env: &Env, bps: u32) -> Royalty {
        Royalty {
            recipients: soroban_sdk::vec![
                env,
                RoyaltyRecipient {
                    recipient: Address::generate(env),
                    basis_points: bps
                }
            ],
            asset_address: None,
        }
    }
    #[ignore]
    #[test]
    fn valid_royalty_passes() {
        let env = Env::default();
        assert!(validate_royalty(&make_royalty(&env, 500)).is_ok());
        assert!(validate_royalty(&make_royalty(&env, 0)).is_ok());
        assert!(validate_royalty(&make_royalty(&env, 10_000)).is_ok());
    }
    #[ignore]
    #[test]
    fn royalty_too_high_fails() {
        let env = Env::default();
        assert_eq!(
            validate_royalty(&make_royalty(&env, 10_001)),
            Err(Error::InvalidBasisPoints)
        );
    }
    #[ignore]
    #[test]
    fn validate_bps_boundary() {
        assert!(validate_royalty_bps(10_000).is_ok());
        assert_eq!(validate_royalty_bps(10_001), Err(Error::InvalidBasisPoints));
    }
    #[ignore]
    #[test]
    fn royalty_above_max_returns_invalid_bps() {
        let env = Env::default();
        assert_eq!(
            validate_royalty_config(&make_royalty(&env, 10_001)),
            Err(RoyaltyValidationError::InvalidRoyaltyBps)
        );
        assert_eq!(
            validate_royalty_bps_config(10_001),
            Err(RoyaltyValidationError::InvalidRoyaltyBps)
        );
    }
    #[ignore]
    #[test]
    fn empty_recipients_returns_invalid_recipient() {
        let env = Env::default();
        let royalty = Royalty {
            recipients: soroban_sdk::vec![&env],
            asset_address: None,
        };
        assert_eq!(
            validate_royalty_config(&royalty),
            Err(RoyaltyValidationError::InvalidRoyaltyRecipient)
        );
        assert_eq!(validate_royalty(&royalty), Err(Error::InvalidBasisPoints));
    }
    #[ignore]
    #[test]
    fn total_bps_above_max_returns_invalid_bps() {
        let env = Env::default();
        let royalty = Royalty {
            recipients: soroban_sdk::vec![
                &env,
                RoyaltyRecipient {
                    recipient: Address::generate(&env),
                    basis_points: 6_000
                },
                RoyaltyRecipient {
                    recipient: Address::generate(&env),
                    basis_points: 6_000
                }
            ],
            asset_address: None,
        };
        assert_eq!(
            validate_royalty_config(&royalty),
            Err(RoyaltyValidationError::InvalidRoyaltyBps)
        );
    }
    #[ignore]
    #[test]
    fn invalid_royalty_state_maps_to_error() {
        assert_eq!(
            RoyaltyValidationError::InvalidRoyaltyState.to_error(),
            Error::InvalidBasisPoints
        );
    }
    #[ignore]
    #[test]
    fn unauthorized_update_maps_to_error() {
        assert_eq!(
            RoyaltyValidationError::UnauthorizedRoyaltyUpdate.to_error(),
            Error::Unauthorized
        );
    }
}
