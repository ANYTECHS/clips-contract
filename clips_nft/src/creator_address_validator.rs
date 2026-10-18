//! Creator address validator — validates creator addresses associated with NFTs.
//!
//! Covers three concerns in layered order:
//!
//! | Layer | Function | What it checks |
//! |-------|----------|----------------|
//! | Format | [`validate_creator_address`] | Rejects self-address (contract cannot be a creator) |
//! | Association | [`verify_creator_association`] | Confirms `creator` matches the on-chain record for `token_id` |
//! | Assignment | [`validate_creator_assignment`] | Format + association; rejects any invalid or mismatched assignment |
//!
//! # Validation rules
//!
//! | Rule | Accepted | Rejected |
//! |------|----------|---------|
//! | Non-self | Any external `Address` | `env.current_contract_address()` |
//! | Token exists | Token has a creator record | Token has no creator record |
//! | Creator match | Provided address equals stored creator | Address differs from stored creator |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Format-only (no storage read):
//! creator_address_validator::validate_creator_address(&env, &creator)?;
//!
//! // Association check (storage read, no format check needed — storage guarantees it):
//! creator_address_validator::verify_creator_association(&env, token_id, &claimed_creator)?;
//!
//! // Full check before accepting a creator assignment:
//! creator_address_validator::validate_creator_assignment(&env, token_id, &creator)?;
//! ```
//!
//! # Errors
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::InvalidAddress`] | `creator` is the contract's own address. |
//! | [`Error::TokenNotFound`]  | No creator record exists for `token_id`. |
//! | [`Error::Unauthorized`]   | Provided `creator` does not match the stored creator. |

use soroban_sdk::{Address, Env};

use crate::creator_storage;
use crate::types::{Error, TokenId};

// ── Format validation ─────────────────────────────────────────────────────────

/// Validate the structural eligibility of `creator` as a creator address.
///
/// A creator address is structurally valid when it is **not** the contract's
/// own address. The contract cannot meaningfully act as a creator: it cannot
/// sign royalty claims, cannot be attributed in off-chain metadata, and
/// accepting it would indicate a logic error in the caller.
///
/// Soroban `Address` values are structurally sound by construction (the host
/// enforces this), so the only structural gate needed here is the self-address
/// check.
///
/// # Errors
/// - [`Error::InvalidAddress`] — `creator == env.current_contract_address()`.
pub fn validate_creator_address(env: &Env, creator: &Address) -> Result<(), Error> {
    if *creator == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

// ── Association verification ──────────────────────────────────────────────────

/// Verify that `claimed_creator` matches the creator stored for `token_id`.
///
/// Looks up the creator record and compares it against `claimed_creator`.
/// Does **not** perform format validation — call [`validate_creator_address`]
/// first if you need both checks, or use [`validate_creator_assignment`] which
/// combines both in one call.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no creator record exists for `token_id`.
/// - [`Error::Unauthorized`]  — `claimed_creator` does not match the stored creator.
pub fn verify_creator_association(
    env: &Env,
    token_id: TokenId,
    claimed_creator: &Address,
) -> Result<(), Error> {
    let stored = creator_storage::get_creator(env, token_id)?; // propagates TokenNotFound
    if *claimed_creator != stored {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

// ── Assignment validation (format + association) ──────────────────────────────

/// Validate `creator` as the creator of `token_id`.
///
/// Combines both checks:
/// 1. [`validate_creator_address`] — `creator` must not be the contract itself.
/// 2. [`verify_creator_association`] — `creator` must match the on-chain record.
///
/// This is the recommended single call for any entry point that receives a
/// `creator` address and needs to confirm it is both structurally sound and
/// genuinely associated with the token.
///
/// # Errors
/// - [`Error::InvalidAddress`] — `creator` is the contract's own address.
/// - [`Error::TokenNotFound`]  — no creator record exists for `token_id`.
/// - [`Error::Unauthorized`]   — `creator` does not match the stored creator.
pub fn validate_creator_assignment(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
) -> Result<(), Error> {
    validate_creator_address(env, creator)?;
    verify_creator_association(env, token_id, creator)?;
    Ok(())
}

// ── Existence probe ───────────────────────────────────────────────────────────

/// Return `true` when `token_id` has a creator record on-chain.
///
/// Non-authoritative probe with no side effects. Useful for conditional
/// branching before committing to a full validation call.
pub fn has_creator(env: &Env, token_id: TokenId) -> bool {
    creator_storage::creator_metadata_exists(env, token_id)
}

/// Return the stored creator for `token_id`, or `TokenNotFound` if absent.
///
/// Thin delegation to [`creator_storage::get_creator`]; exposed here so
/// callers that import only this module can retrieve the creator without
/// depending on `creator_storage` directly.
///
/// # Errors
/// - [`Error::TokenNotFound`] — no creator record exists for `token_id`.
pub fn get_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    creator_storage::get_creator(env, token_id)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::creator_storage;
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

    fn seed_creator(env: &Env, token_id: TokenId, creator: &Address) {
        creator_storage::set_creator(env, token_id, creator);
    }

    // ── validate_creator_address ──────────────────────────────────────────────

    #[test]
    fn format_accepts_external_address() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert!(validate_creator_address(env, &creator).is_ok());
        });
    }

    #[test]
    fn format_rejects_contract_self_address() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_creator_address(env, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn format_accepts_multiple_distinct_addresses() {
        with_contract(|env| {
            let a = Address::generate(env);
            let b = Address::generate(env);
            assert!(validate_creator_address(env, &a).is_ok());
            assert!(validate_creator_address(env, &b).is_ok());
            assert_ne!(a, b);
        });
    }

    #[test]
    fn format_is_deterministic_for_same_address() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert!(validate_creator_address(env, &creator).is_ok());
            assert!(validate_creator_address(env, &creator).is_ok());
        });
    }

    // ── verify_creator_association ────────────────────────────────────────────

    #[test]
    fn association_accepts_matching_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            seed_creator(env, 1, &creator);
            assert!(verify_creator_association(env, 1, &creator).is_ok());
        });
    }

    #[test]
    fn association_rejects_wrong_address() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let imposter = Address::generate(env);
            seed_creator(env, 1, &creator);
            assert_eq!(
                verify_creator_association(env, 1, &imposter),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn association_rejects_missing_token() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert_eq!(
                verify_creator_association(env, 999, &creator),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn association_is_per_token_and_does_not_bleed() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            seed_creator(env, 1, &alice);
            seed_creator(env, 2, &bob);

            assert!(verify_creator_association(env, 1, &alice).is_ok());
            assert!(verify_creator_association(env, 2, &bob).is_ok());
            // Cross-checks must fail
            assert_eq!(
                verify_creator_association(env, 1, &bob),
                Err(Error::Unauthorized)
            );
            assert_eq!(
                verify_creator_association(env, 2, &alice),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn association_reflects_updated_creator_record() {
        with_contract(|env| {
            let original = Address::generate(env);
            let updated = Address::generate(env);
            seed_creator(env, 1, &original);

            // Overwrite with a new creator
            seed_creator(env, 1, &updated);

            assert_eq!(
                verify_creator_association(env, 1, &original),
                Err(Error::Unauthorized),
                "old creator must no longer be accepted after update"
            );
            assert!(verify_creator_association(env, 1, &updated).is_ok());
        });
    }

    // ── validate_creator_assignment ───────────────────────────────────────────

    #[test]
    fn assignment_accepts_valid_creator_for_existing_token() {
        with_contract(|env| {
            let creator = Address::generate(env);
            seed_creator(env, 1, &creator);
            assert!(validate_creator_assignment(env, 1, &creator).is_ok());
        });
    }

    #[test]
    fn assignment_rejects_contract_self_address() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            // Seed with the contract address to isolate the format gate
            seed_creator(env, 1, &contract_addr);
            assert_eq!(
                validate_creator_assignment(env, 1, &contract_addr),
                Err(Error::InvalidAddress),
                "format check must fire before storage lookup"
            );
        });
    }

    #[test]
    fn assignment_rejects_missing_token() {
        with_contract(|env| {
            let creator = Address::generate(env);
            assert_eq!(
                validate_creator_assignment(env, 999, &creator),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn assignment_rejects_mismatched_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let imposter = Address::generate(env);
            seed_creator(env, 1, &creator);
            assert_eq!(
                validate_creator_assignment(env, 1, &imposter),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn assignment_format_check_runs_before_association_check() {
        // Even when the token exists, the format gate must fire first.
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            seed_creator(env, 1, &contract_addr);
            assert_eq!(
                validate_creator_assignment(env, 1, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    // ── has_creator ───────────────────────────────────────────────────────────

    #[test]
    fn has_creator_returns_true_when_record_exists() {
        with_contract(|env| {
            let creator = Address::generate(env);
            seed_creator(env, 1, &creator);
            assert!(has_creator(env, 1));
        });
    }

    #[test]
    fn has_creator_returns_false_when_no_record() {
        with_contract(|env| {
            assert!(!has_creator(env, 99));
        });
    }

    #[test]
    fn has_creator_returns_false_after_removal() {
        with_contract(|env| {
            let creator = Address::generate(env);
            seed_creator(env, 1, &creator);
            creator_storage::remove_creator_metadata(env, 1);
            assert!(!has_creator(env, 1));
        });
    }

    // ── get_creator ───────────────────────────────────────────────────────────

    #[test]
    fn get_creator_returns_stored_address() {
        with_contract(|env| {
            let creator = Address::generate(env);
            seed_creator(env, 1, &creator);
            assert_eq!(get_creator(env, 1).unwrap(), creator);
        });
    }

    #[test]
    fn get_creator_returns_token_not_found_when_absent() {
        with_contract(|env| {
            assert_eq!(get_creator(env, 404), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn get_creator_reflects_most_recent_write() {
        with_contract(|env| {
            let first = Address::generate(env);
            let second = Address::generate(env);
            seed_creator(env, 1, &first);
            seed_creator(env, 1, &second);
            assert_eq!(get_creator(env, 1).unwrap(), second);
        });
    }
}
