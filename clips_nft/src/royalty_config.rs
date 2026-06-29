use soroban_sdk::{contracttype, Address};

use crate::Error;

/// Maximum allowed royalty in basis points (100% = 10_000).
pub const MAX_ROYALTY_BPS: u32 = 10_000;

/// Stores royalty recipient information and royalty percentage for a clip.
///
/// `royalty_bps` is expressed in basis points where 10_000 = 100%.
/// Call [`RoyaltyConfig::validate`] after construction to ensure invariants.
#[contracttype]
#[derive(Clone)]
pub struct RoyaltyConfig {
    /// Address that receives the royalty payment.
    pub recipient: Address,
    /// Royalty percentage in basis points (0 – 10_000).
    pub royalty_bps: u32,
}

impl RoyaltyConfig {
    /// Returns `Err(Error::RoyaltyTooHigh)` when `royalty_bps > 10_000`.
    pub fn validate(&self) -> Result<(), Error> {
        if self.royalty_bps > MAX_ROYALTY_BPS {
            return Err(Error::RoyaltyTooHigh);
        }
        Ok(())
    }
}
