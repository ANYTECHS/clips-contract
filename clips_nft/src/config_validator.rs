//! Configuration validator (Issue #483).
//!
//! Reusable validator for every configuration update.
//! Validates fees, addresses, URIs, collection limits, and royalty percentages.

use soroban_sdk::{Address, Env, String};

use crate::default_royalty::MAX_ROYALTY_BPS;
use crate::platform_fee::MAX_PLATFORM_FEE_BPS;
use crate::types::Error;

use crate::storage_constants::{MAX_BATCH_MINT_SIZE_LIMIT, MAX_BATCH_TRANSFER_SIZE_LIMIT};
pub use crate::storage_constants::MAX_COLLECTION_LIMIT;

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

/// Validate a configuration address.
///
/// Rejects the contract's own address and any blacklisted address with
/// [`Error::InvalidAddress`].
pub fn validate_address(env: &Env, addr: &Address) -> Result<(), Error> {
    if *addr == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    if crate::blacklist::is_blacklisted(env, addr) {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate that an address is present (non-None check for Option<Address>).
///
/// For direct Address values use [`validate_address`] to also check contract
/// self-address and blacklist status.
pub fn validate_address_present(addr: &Option<Address>) -> Result<(), Error> {
    if addr.is_none() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate a metadata URI string.
///
/// Rejects empty strings and unsupported schemes; must start with
/// `https://`, `http://`, `ipfs://`, or `ar://`.
pub fn validate_uri(uri: &String) -> Result<(), Error> {
    crate::chain_config::metadata_base_uri::validate_metadata_base_uri(uri)
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

/// Validate a batch mint size limit.
///
/// Must be in range `[1, MAX_BATCH_MINT_SIZE_LIMIT]`.
pub fn validate_batch_mint_size(size: u32) -> Result<(), Error> {
    if size == 0 || size > MAX_BATCH_MINT_SIZE_LIMIT {
        return Err(Error::InvalidLimit);
    }
    Ok(())
}

/// Validate a batch transfer size limit.
///
/// Must be in range `[1, MAX_BATCH_TRANSFER_SIZE_LIMIT]`.
pub fn validate_batch_transfer_size(size: u32) -> Result<(), Error> {
    if size == 0 || size > MAX_BATCH_TRANSFER_SIZE_LIMIT {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}

/// Validate a full Config struct before persisting.
///
/// Checks platform fee, default royalty, owner address, batch mint size,
/// batch transfer size, collection limit, combined deductions, and version.
pub fn validate_config(env: &Env, config: &crate::config::Config) -> Result<(), Error> {
    // Validate platform fee range
    validate_fee(config.platform_fee_bps)?;

    // Validate default royalty range
    validate_royalty_bps(config.default_royalty_bps)?;

    // Validate owner address
    validate_address(env, &config.owner)?;

    // Validate batch mint size
    validate_batch_mint_size(config.max_batch_mint_size)?;

    // Validate batch transfer size
    validate_batch_transfer_size(config.max_batch_transfer_size)?;

    // Validate collection limit
    validate_collection_limit(config.max_collection_size)?;

    // Validate combined royalty + platform fee don't exceed 100%
    crate::transaction_deduction_validator::validate_total_deduction_bps(
        config.default_royalty_bps,
        config.platform_fee_bps,
    )?;

    // Version must be > 0
    if config.version == 0 {
        return Err(Error::InvalidLimit);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn valid_config(owner: Address) -> Config {
        Config {
            owner,
            version: 1,
            platform_fee_bps: 100,
            default_royalty_bps: 500,
            paused: false,
            max_batch_mint_size: 50,
            max_batch_transfer_size: 50,
            max_collection_size: 10_000,
        }
    }

    #[test]
    fn validate_fee_accepts_bounds_and_rejects_excess() {
        assert!(validate_fee(0).is_ok());
        assert!(validate_fee(MAX_PLATFORM_FEE_BPS).is_ok());
        assert_eq!(validate_fee(MAX_PLATFORM_FEE_BPS + 1), Err(Error::InvalidFee));
    }

    #[test]
    fn validate_royalty_bps_accepts_bounds_and_rejects_excess() {
        assert!(validate_royalty_bps(0).is_ok());
        assert!(validate_royalty_bps(MAX_ROYALTY_BPS).is_ok());
        assert_eq!(
            validate_royalty_bps(MAX_ROYALTY_BPS + 1),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn validate_address_rejects_contract_and_blacklisted_addresses() {
        with_contract(|env| {
            let valid = Address::generate(env);
            assert!(validate_address(env, &valid).is_ok());

            let self_addr = env.current_contract_address();
            assert_eq!(validate_address(env, &self_addr), Err(Error::InvalidAddress));

            let blacklisted = Address::generate(env);
            crate::blacklist::add_wallet(env, &blacklisted);
            assert_eq!(
                validate_address(env, &blacklisted),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn validate_address_present_checks_option() {
        let env = Env::default();
        let addr = Address::generate(&env);
        assert!(validate_address_present(&Some(addr)).is_ok());
        assert_eq!(validate_address_present(&None), Err(Error::InvalidAddress));
    }

    #[test]
    fn validate_uri_accepts_supported_schemes_and_rejects_invalid() {
        let env = Env::default();
        assert!(validate_uri(&String::from_str(&env, "https://example.com/1.json")).is_ok());
        assert!(validate_uri(&String::from_str(&env, "ipfs://QmHash/1.json")).is_ok());
        assert!(validate_uri(&String::from_str(&env, "ar://TxId/1.json")).is_ok());
        assert_eq!(
            validate_uri(&String::from_str(&env, "")),
            Err(Error::InvalidURI)
        );
        assert_eq!(
            validate_uri(&String::from_str(&env, "ftp://example.com/1.json")),
            Err(Error::InvalidURI)
        );
    }

    #[test]
    fn validate_collection_limit_checks_bounds() {
        assert!(validate_collection_limit(1).is_ok());
        assert!(validate_collection_limit(MAX_COLLECTION_LIMIT).is_ok());
        assert_eq!(validate_collection_limit(0), Err(Error::InvalidLimit));
        assert_eq!(
            validate_collection_limit(MAX_COLLECTION_LIMIT + 1),
            Err(Error::InvalidLimit)
        );
    }

    #[test]
    fn validate_config_checks_all_fields() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let cfg = valid_config(owner.clone());
            assert!(validate_config(env, &cfg).is_ok());

            let mut bad_fee = valid_config(owner.clone());
            bad_fee.platform_fee_bps = MAX_PLATFORM_FEE_BPS + 1;
            assert_eq!(validate_config(env, &bad_fee), Err(Error::InvalidFee));

            let mut bad_limit = valid_config(owner.clone());
            bad_limit.max_collection_size = 0;
            assert_eq!(validate_config(env, &bad_limit), Err(Error::InvalidLimit));

            let mut bad_owner = valid_config(owner);
            bad_owner.owner = env.current_contract_address();
            assert_eq!(validate_config(env, &bad_owner), Err(Error::InvalidAddress));
        });
    }
}
