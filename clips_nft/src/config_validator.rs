//! Configuration validator (Issue #483).
//!
//! Reusable validator for every configuration update.
//! Validates fees, addresses, URIs, collection limits, and royalty percentages.

use soroban_sdk::{Address, Env, String};

use crate::default_royalty::MAX_ROYALTY_BPS;
use crate::platform_fee::MAX_PLATFORM_FEE_BPS;
use crate::types::Error;

/// Maximum collection size limit (prevents unbounded storage growth).
pub const MAX_COLLECTION_LIMIT: u32 = 100_000;

/// Validate a platform fee value in basis points.
///
/// Must be in range `[0, MAX_PLATFORM_FEE_BPS]` (0–10 %).
pub fn validate_fee(fee_bps: u32) -> Result<(), Error> {
    if fee_bps > MAX_PLATFORM_FEE_BPS {
        return Err(Error::InvalidFee);
    }
    Ok(())
}

/// Validate a royalty percentage in basis points.
///
/// Must be in range `[0, MAX_ROYALTY_BPS]` (0–100 %).
pub fn validate_royalty_bps(bps: u32) -> Result<(), Error> {
    if bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    Ok(())
}

/// Validate that an address is present (non-None check for Option<Address>).
///
/// For direct Address values this always succeeds since Soroban addresses
/// are structurally valid by construction. Use this for Option<Address> fields.
pub fn validate_address_present(addr: &Option<Address>) -> Result<(), Error> {
    if addr.is_none() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate a metadata URI string.
///
/// Rejects empty strings.
pub fn validate_uri(uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::InvalidURI);
    }
    Ok(())
}

/// Validate a collection limit.
///
/// Must be in range `[1, MAX_COLLECTION_LIMIT]`.
pub fn validate_collection_limit(limit: u32) -> Result<(), Error> {
    if limit == 0 || limit > MAX_COLLECTION_LIMIT {
        return Err(Error::InvalidLimit);
    }
    Ok(())
}

/// Validate a full Config struct before persisting.
///
/// Checks platform fee, default royalty, and owner address.
pub fn validate_config(_env: &Env, config: &crate::config::Config) -> Result<(), Error> {
    // Validate platform fee range
    validate_fee(config.platform_fee_bps)?;

    // Validate default royalty range
    validate_royalty_bps(config.default_royalty_bps)?;

    // Owner address is always valid by Soroban type system — no extra check needed.
    // Version must be > 0
    if config.version == 0 {
        return Err(Error::InvalidLimit);
    }

    Ok(())
}
