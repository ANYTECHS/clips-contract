//! Validation Helper Utilities (Issue #1087).
//!
//! Shared, reusable helpers for common validation operations used across the
//! contract's validators and guards.  Centralising these eliminates duplicated
//! validation logic and ensures every module uses the canonical error
//! definitions from [`crate::types::Error`].
//!
//! # Available helpers
//!
//! | Helper | Validates |
//! |--------|-----------|
//! | [`require_positive_u32`] | u32 must be > 0. |
//! | [`validate_basis_points`] | BPS within `[0, max_bps]`. |
//! | [`validate_range_u32`] | u32 within `[min, max]`. |
//! | [`validate_not_contract_address`] | Address ≠ current contract. |
//! | [`validate_not_blacklisted`] | Address not on blacklist. |
//! | [`validate_option_address`] | `Option<Address>` is `Some(_)`. |
//! | [`validate_non_empty_string`] | String length > 0. |
//! | [`validate_string_length`] | String length ≤ max. |
//! | [`validate_uri_protocol`] | URI starts with `https://`, `ipfs://`, or `ar://`. |
//! | [`validate_token_id_exists`] | Token has an ownership record. |
//! | [`validate_positive_i128`] | i128 > 0. |

use soroban_sdk::{Address, Env, String};

use crate::blacklist;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Numeric helpers ──────────────────────────────────────────────────────────

/// Validate that `value` is strictly greater than zero.
///
/// # Errors
/// Returns [`Error::InvalidConfig`] when `value == 0`.
pub fn require_positive_u32(value: u32) -> Result<(), Error> {
    if value == 0 {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}

/// Validate that `bps` is within `[0, max_bps]` (inclusive).
///
/// Basis points represent a percentage (10 000 bps = 100 %).  Values above
/// `max_bps` are rejected to prevent economically invalid configurations.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when `bps > max_bps`.
pub fn validate_basis_points(bps: u32, max_bps: u32) -> Result<(), Error> {
    if bps > max_bps {
        return Err(Error::InvalidBasisPoints);
    }
    Ok(())
}

/// Validate that `value` is within `[min, max]` (inclusive).
///
/// # Errors
/// Returns [`Error::InvalidConfig`] when `value < min || value > max`.
pub fn validate_range_u32(value: u32, min: u32, max: u32) -> Result<(), Error> {
    if value < min || value > max {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}

/// Validate that an `i128` value is strictly positive (> 0).
///
/// Useful for prices, amounts, and any field that must be non-zero positive.
///
/// # Errors
/// Returns [`Error::InvalidSalePrice`] when `value <= 0`.
pub fn validate_positive_i128(value: i128) -> Result<(), Error> {
    if value <= 0 {
        return Err(Error::InvalidSalePrice);
    }
    Ok(())
}

// ─── Address helpers ──────────────────────────────────────────────────────────

/// Validate that `address` is not the contract's own address.
///
/// The contract cannot own tokens or receive payments directed to itself.
///
/// # Errors
/// Returns [`Error::InvalidAddress`] when `address == current_contract_address`.
pub fn validate_not_contract_address(env: &Env, address: &Address) -> Result<(), Error> {
    if *address == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate that `address` is not on the contract blacklist.
///
/// # Errors
/// Returns [`Error::InvalidAddress`] when `address` has been blacklisted.
pub fn validate_not_blacklisted(env: &Env, address: &Address) -> Result<(), Error> {
    if blacklist::is_blacklisted(env, address) {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Validate that an `Option<Address>` is `Some(_)`.
///
/// # Errors
/// Returns [`Error::InvalidAddress`] when `addr` is `None`.
pub fn validate_option_address(addr: &Option<Address>) -> Result<(), Error> {
    if addr.is_none() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

// ─── String / URI helpers ─────────────────────────────────────────────────────

/// Validate that `s` is non-empty (length > 0).
///
/// # Errors
/// Returns [`Error::InvalidURI`] when `s.len() == 0`.
pub fn validate_non_empty_string(s: &String) -> Result<(), Error> {
    if s.len() == 0 {
        return Err(Error::InvalidURI);
    }
    Ok(())
}

/// Validate that `s` does not exceed `max_len` characters.
///
/// # Errors
/// Returns [`Error::MetadataSizeTooLarge`] when `s.len() > max_len`.
pub fn validate_string_length(s: &String, max_len: u32) -> Result<(), Error> {
    if s.len() > max_len {
        return Err(Error::MetadataSizeTooLarge);
    }
    Ok(())
}

/// Validate that a URI begins with a supported protocol scheme.
///
/// Accepted schemes (in evaluation order): `https://`, `ipfs://`, `ar://`.
///
/// # Errors
/// Returns [`Error::MalformedUrl`] when the URI is empty.
/// Returns [`Error::UnsupportedProtocol`] when no supported scheme matches.
pub fn validate_uri_protocol(env: &Env, uri: &String) -> Result<(), Error> {
    if uri.len() == 0 {
        return Err(Error::MalformedUrl);
    }
    let schemes: &[&str] = &["https://", "ipfs://", "ar://"];
    for scheme in schemes {
        let prefix = soroban_sdk::String::from_str(env, scheme);
        if string_starts_with(uri, &prefix) {
            return Ok(());
        }
    }
    Err(Error::UnsupportedProtocol)
}

// ─── Token helpers ────────────────────────────────────────────────────────────

/// Validate that `token_id` has an on-chain ownership record.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] when no ownership record exists for
/// `token_id`.
pub fn validate_token_id_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if !token_owner_storage::has_owner(env, token_id) {
        return Err(Error::TokenNotFound);
    }
    Ok(())
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Return `true` if `haystack` begins with `prefix` (byte-level comparison).
fn string_starts_with(haystack: &String, prefix: &String) -> bool {
    let prefix_len = prefix.len();
    if haystack.len() < prefix_len {
        return false;
    }
    for i in 0..prefix_len {
        if haystack.get(i) != prefix.get(i) {
            return false;
        }
    }
    true
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blacklist;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── require_positive_u32 ──────────────────────────────────────────────────

    #[test]
    fn positive_u32_passes_for_non_zero_value() {
        assert!(require_positive_u32(1).is_ok());
        assert!(require_positive_u32(u32::MAX).is_ok());
    }

    #[test]
    fn positive_u32_fails_for_zero() {
        assert_eq!(require_positive_u32(0), Err(Error::InvalidConfig));
    }

    // ── validate_basis_points ─────────────────────────────────────────────────

    #[test]
    fn basis_points_passes_at_zero_and_maximum() {
        assert!(validate_basis_points(0, 10_000).is_ok());
        assert!(validate_basis_points(10_000, 10_000).is_ok());
    }

    #[test]
    fn basis_points_fails_above_maximum() {
        assert_eq!(
            validate_basis_points(10_001, 10_000),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn basis_points_respects_custom_max() {
        assert!(validate_basis_points(1_000, 1_000).is_ok());
        assert_eq!(
            validate_basis_points(1_001, 1_000),
            Err(Error::InvalidBasisPoints)
        );
    }

    // ── validate_range_u32 ────────────────────────────────────────────────────

    #[test]
    fn range_passes_at_boundaries_and_midpoint() {
        assert!(validate_range_u32(1, 1, 10).is_ok());
        assert!(validate_range_u32(5, 1, 10).is_ok());
        assert!(validate_range_u32(10, 1, 10).is_ok());
    }

    #[test]
    fn range_fails_below_minimum() {
        assert_eq!(validate_range_u32(0, 1, 10), Err(Error::InvalidConfig));
    }

    #[test]
    fn range_fails_above_maximum() {
        assert_eq!(validate_range_u32(11, 1, 10), Err(Error::InvalidConfig));
    }

    // ── validate_positive_i128 ────────────────────────────────────────────────

    #[test]
    fn positive_i128_passes_for_positive_value() {
        assert!(validate_positive_i128(1).is_ok());
        assert!(validate_positive_i128(i128::MAX).is_ok());
    }

    #[test]
    fn positive_i128_fails_for_zero() {
        assert_eq!(validate_positive_i128(0), Err(Error::InvalidSalePrice));
    }

    #[test]
    fn positive_i128_fails_for_negative_value() {
        assert_eq!(validate_positive_i128(-1), Err(Error::InvalidSalePrice));
    }

    // ── validate_not_contract_address ─────────────────────────────────────────

    #[test]
    fn non_contract_address_passes() {
        with_contract(|env| {
            let addr = Address::generate(env);
            assert!(validate_not_contract_address(env, &addr).is_ok());
        });
    }

    #[test]
    fn contract_address_is_rejected() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(
                validate_not_contract_address(env, &contract),
                Err(Error::InvalidAddress)
            );
        });
    }

    // ── validate_not_blacklisted ──────────────────────────────────────────────

    #[test]
    fn clean_address_passes_blacklist_check() {
        with_contract(|env| {
            let addr = Address::generate(env);
            assert!(validate_not_blacklisted(env, &addr).is_ok());
        });
    }

    #[test]
    fn blacklisted_address_is_rejected() {
        with_contract(|env| {
            let addr = Address::generate(env);
            blacklist::add_wallet(env, &addr);
            assert_eq!(
                validate_not_blacklisted(env, &addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn removed_from_blacklist_passes_check() {
        with_contract(|env| {
            let addr = Address::generate(env);
            blacklist::add_wallet(env, &addr);
            blacklist::remove_wallet(env, &addr);
            assert!(validate_not_blacklisted(env, &addr).is_ok());
        });
    }

    // ── validate_option_address ───────────────────────────────────────────────

    #[test]
    fn some_address_passes() {
        let env = Env::default();
        let addr = Address::generate(&env);
        assert!(validate_option_address(&Some(addr)).is_ok());
    }

    #[test]
    fn none_address_fails() {
        assert_eq!(validate_option_address(&None), Err(Error::InvalidAddress));
    }

    // ── validate_non_empty_string ─────────────────────────────────────────────

    #[test]
    fn non_empty_string_passes() {
        let env = Env::default();
        let s = String::from_str(&env, "hello");
        assert!(validate_non_empty_string(&s).is_ok());
    }

    #[test]
    fn empty_string_fails() {
        let env = Env::default();
        let s = String::from_str(&env, "");
        assert_eq!(validate_non_empty_string(&s), Err(Error::InvalidURI));
    }

    // ── validate_string_length ────────────────────────────────────────────────

    #[test]
    fn string_at_max_length_passes() {
        let env = Env::default();
        let s = String::from_str(&env, "hello"); // 5 chars
        assert!(validate_string_length(&s, 5).is_ok());
    }

    #[test]
    fn string_exceeding_max_length_fails() {
        let env = Env::default();
        let s = String::from_str(&env, "hello world"); // 11 chars
        assert_eq!(
            validate_string_length(&s, 5),
            Err(Error::MetadataSizeTooLarge)
        );
    }

    // ── validate_uri_protocol ─────────────────────────────────────────────────

    #[test]
    fn https_uri_passes() {
        with_contract(|env| {
            let uri = String::from_str(env, "https://example.com/nft.json");
            assert!(validate_uri_protocol(env, &uri).is_ok());
        });
    }

    #[test]
    fn ipfs_uri_passes() {
        with_contract(|env| {
            let uri = String::from_str(env, "ipfs://QmExampleHash");
            assert!(validate_uri_protocol(env, &uri).is_ok());
        });
    }

    #[test]
    fn arweave_uri_passes() {
        with_contract(|env| {
            let uri = String::from_str(env, "ar://ExampleTxId");
            assert!(validate_uri_protocol(env, &uri).is_ok());
        });
    }

    #[test]
    fn unsupported_scheme_fails() {
        with_contract(|env| {
            let uri = String::from_str(env, "ftp://example.com");
            assert_eq!(
                validate_uri_protocol(env, &uri),
                Err(Error::UnsupportedProtocol)
            );
        });
    }

    #[test]
    fn empty_uri_returns_malformed_url() {
        with_contract(|env| {
            let uri = String::from_str(env, "");
            assert_eq!(validate_uri_protocol(env, &uri), Err(Error::MalformedUrl));
        });
    }

    // ── validate_token_id_exists ──────────────────────────────────────────────

    #[test]
    fn minted_token_passes_existence_check() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::assign_owner(env, 1, &owner, 1).unwrap();
            assert!(validate_token_id_exists(env, 1).is_ok());
        });
    }

    #[test]
    fn unminted_token_fails_existence_check() {
        with_contract(|env| {
            assert_eq!(validate_token_id_exists(env, 999), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn removed_token_fails_existence_check() {
        with_contract(|env| {
            let owner = Address::generate(env);
            token_owner_storage::assign_owner(env, 1, &owner, 1).unwrap();
            token_owner_storage::remove_owner(env, 1);
            assert_eq!(validate_token_id_exists(env, 1), Err(Error::TokenNotFound));
        });
    }
}
