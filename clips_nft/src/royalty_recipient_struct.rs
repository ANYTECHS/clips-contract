//! Reusable `RoyaltyRecipient` struct and validation (issue #779).
//!
//! A [`RoyaltyRecipient`] represents a single wallet that receives a share of
//! the royalty payment on every secondary sale of a ClipCash NFT.
//!
//! # Fields
//! - `recipient`    — Stellar wallet address that receives the royalty.
//! - `basis_points` — Royalty share for this recipient in basis points
//!                    (100 bps = 1 %, maximum 10 000 bps = 100 %).
//!
//! # Multi-recipient splits
//! A [`crate::types::Royalty`] holds a `Vec<RoyaltyRecipient>`, so the total
//! royalty can be shared across multiple wallets (e.g. creator + platform).
//! Each [`RoyaltyRecipient`] is validated independently; the caller is
//! responsible for ensuring the sum of all `basis_points` values does not
//! exceed the contract maximum.
//!
//! # Serialization
//! The struct derives [`contracttype`] via [`crate::types`], which makes it
//! automatically serializable by the Soroban SDK for persistent storage and
//! cross-contract calls.
//!
//! # Validation
//! Use [`validate_royalty_recipient_struct`] to check that:
//! - `basis_points` is within `0..=MAX_ROYALTY_BPS` (0 %–100 %).
//! - `recipient` is not the contract's own address.
//!
//! # Example
//! ```ignore
//! use clips_nft::{RoyaltyRecipient};
//! use clips_nft::royalty_recipient_struct::validate_royalty_recipient_struct;
//!
//! let r = RoyaltyRecipient { recipient: creator_address, basis_points: 500 };
//! validate_royalty_recipient_struct(&env, &r)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::royalty_recipient_validator::validate_royalty_recipient;
use crate::storage_constants::MAX_ROYALTY_BPS;
use crate::types::{Error, RoyaltyRecipient};

// ─── Validation ───────────────────────────────────────────────────────────────

/// Validate a [`RoyaltyRecipient`] value.
///
/// Checks:
/// 1. `basis_points` must be in `0..=MAX_ROYALTY_BPS` (10 000 = 100 %).
/// 2. `recipient` must not be the current contract address.
///
/// # Errors
/// - [`Error::InvalidBasisPoints`] — `basis_points > MAX_ROYALTY_BPS`.
/// - [`Error::InvalidRecipient`]   — `recipient` is the contract itself.
pub fn validate_royalty_recipient_struct(env: &Env, r: &RoyaltyRecipient) -> Result<(), Error> {
    if r.basis_points > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    validate_royalty_recipient(env, &r.recipient)?;
    Ok(())
}

/// Convenience constructor that validates on creation.
///
/// Returns `Ok(RoyaltyRecipient)` if all invariants hold, or the first
/// validation error encountered.
///
/// # Arguments
/// * `env`          — Soroban execution environment (needed for address check).
/// * `recipient`    — Wallet address that will receive royalty payments.
/// * `basis_points` — Royalty share in basis points (0–10 000).
pub fn new_royalty_recipient(
    env: &Env,
    recipient: Address,
    basis_points: u32,
) -> Result<RoyaltyRecipient, Error> {
    let r = RoyaltyRecipient {
        recipient,
        basis_points,
    };
    validate_royalty_recipient_struct(env, &r)?;
    Ok(r)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── validate_royalty_recipient_struct ─────────────────────────────────────

    #[test]
    fn valid_recipient_and_bps_passes() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 500,
            };
            assert!(validate_royalty_recipient_struct(env, &r).is_ok());
        });
    }

    #[test]
    fn zero_bps_is_valid() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 0,
            };
            assert!(validate_royalty_recipient_struct(env, &r).is_ok());
        });
    }

    #[test]
    fn max_bps_is_valid() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: MAX_ROYALTY_BPS,
            };
            assert!(validate_royalty_recipient_struct(env, &r).is_ok());
        });
    }

    #[test]
    fn above_max_bps_returns_invalid_basis_points() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: MAX_ROYALTY_BPS + 1,
            };
            assert_eq!(
                validate_royalty_recipient_struct(env, &r),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn u32_max_bps_returns_invalid_basis_points() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: u32::MAX,
            };
            assert_eq!(
                validate_royalty_recipient_struct(env, &r),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn contract_self_address_returns_invalid_recipient() {
        with_contract(|env| {
            let r = RoyaltyRecipient {
                recipient: env.current_contract_address(),
                basis_points: 500,
            };
            assert_eq!(
                validate_royalty_recipient_struct(env, &r),
                Err(Error::InvalidRecipient)
            );
        });
    }

    // ── new_royalty_recipient ─────────────────────────────────────────────────

    #[test]
    fn constructor_returns_struct_on_valid_input() {
        with_contract(|env| {
            let addr = Address::generate(env);
            let r = new_royalty_recipient(env, addr.clone(), 750).unwrap();
            assert_eq!(r.recipient, addr);
            assert_eq!(r.basis_points, 750);
        });
    }

    #[test]
    fn constructor_rejects_invalid_bps() {
        with_contract(|env| {
            let result = new_royalty_recipient(env, Address::generate(env), 20_000);
            assert_eq!(result, Err(Error::InvalidBasisPoints));
        });
    }

    #[test]
    fn constructor_rejects_contract_address() {
        with_contract(|env| {
            let result = new_royalty_recipient(env, env.current_contract_address(), 500);
            assert_eq!(result, Err(Error::InvalidRecipient));
        });
    }

    // ── Struct properties ─────────────────────────────────────────────────────

    #[test]
    fn struct_fields_are_accessible() {
        with_contract(|env| {
            let addr = Address::generate(env);
            let r = RoyaltyRecipient {
                recipient: addr.clone(),
                basis_points: 1_000,
            };
            assert_eq!(r.recipient, addr);
            assert_eq!(r.basis_points, 1_000);
        });
    }

    #[test]
    fn struct_can_be_cloned() {
        with_contract(|env| {
            let addr = Address::generate(env);
            let r = RoyaltyRecipient {
                recipient: addr.clone(),
                basis_points: 300,
            };
            let cloned = r.clone();
            assert_eq!(cloned.recipient, r.recipient);
            assert_eq!(cloned.basis_points, r.basis_points);
        });
    }

    #[test]
    fn two_recipients_are_independent() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            let ra = new_royalty_recipient(env, a.clone(), 300).unwrap();
            let rb = new_royalty_recipient(env, b.clone(), 200).unwrap();
            assert_ne!(ra.recipient, rb.recipient);
            assert_ne!(ra.basis_points, rb.basis_points);
        });
    }
}
