//! Caller identity validator — validates caller identity before executing
//! operations that require authentication.
//!
//! This module is the single entry point for caller identity validation across
//! all contract operations. It answers two questions:
//!
//! 1. **Is this caller structurally valid?** (non-self, non-blacklisted)
//! 2. **Does this caller hold one of the required roles?** (admin, owner,
//!    creator, minter, or any combination via [`CallerRole`])
//!
//! # Design
//!
//! Three layers of increasing strictness:
//!
//! | Layer | Function | What it checks |
//! |-------|----------|----------------|
//! | Retrieval | [`get_caller`] | Returns the caller address unchanged |
//! | Structural validation | [`validate_caller`] | Rejects self-calls and blacklisted addresses |
//! | Role authorization | [`require_caller_role`] | Structural check + role + `require_auth` |
//!
//! All higher-level entry points call [`validate_caller`] before anything else,
//! so the self-call and blacklist gates run exactly once and are never skipped.
//!
//! # Role hierarchy
//!
//! [`CallerRole`] enumerates every first-class identity the contract
//! recognizes:
//!
//! | Role | Resolution |
//! |------|-----------|
//! | `Admin` | Contract-level admin at `DataKey::Admin` or any registered multi-admin |
//! | `Owner(token_id)` | Current owner of the token |
//! | `Creator(token_id)` | Original creator of the token |
//! | `Minter` | Address that holds the `ApprovedMinter` role |
//! | `Any` | Any structurally valid, non-blacklisted caller |
//!
//! # Guard composition
//!
//! Because every function returns `Result<(), Error>`, they compose naturally
//! with the existing [`crate::guard_composition`] framework:
//!
//! ```rust,ignore
//! guard_composition::sequence(
//!     pause_guard::require_not_paused(&env),
//!     caller_validator::require_caller_role(&env, &caller, CallerRole::Admin),
//! )?;
//! ```
//!
//! # Errors (in evaluation order)
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::NotInitialized`] | Contract has no admin stored (role checks only). |
//! | [`Error::InvalidAddress`] | Caller is the contract's own address. |
//! | [`Error::Unauthorized`] | Caller is blacklisted. |
//! | [`Error::Unauthorized`] | Caller does not hold the required role. |

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::blacklist;
use crate::creator_storage;
use crate::mint_authorization;
use crate::token_owner_storage;
use crate::types::{DataKey, Error, TokenId};

// ─── Role enum ────────────────────────────────────────────────────────────────

/// The role a caller must hold for an operation to be authorized.
///
/// Passed to [`require_caller_role`] to express which identity check to
/// enforce. Role checks always occur **after** the structural [`validate_caller`]
/// gates (self-call + blacklist).
#[derive(Clone)]
pub enum CallerRole {
    /// Caller must be the contract-level admin or a registered multi-admin.
    Admin,
    /// Caller must be the current owner of the given token.
    Owner(TokenId),
    /// Caller must be the original creator of the given token.
    Creator(TokenId),
    /// Caller must hold the `ApprovedMinter` role.
    Minter,
    /// Any structurally valid, non-blacklisted caller is accepted.
    ///
    /// Use when you only need the self-call and blacklist gates but do not
    /// restrict to a specific role.
    Any,
}

// ─── Retrieval ────────────────────────────────────────────────────────────────

/// Return `caller` unchanged.
///
/// This is intentionally a thin wrapper. In Soroban there is no ambient
/// `msg.sender`; callers are passed explicitly. This function standardizes
/// how entry points retrieve the caller and serves as the documented hook
/// point for any future caller enrichment (e.g. aliasing, logging).
///
/// # Example
///
/// ```rust,ignore
/// let caller = caller_validator::get_caller(&env, &raw_address);
/// caller_validator::validate_caller(&env, &caller)?;
/// ```
pub fn get_caller(_env: &Env, address: &Address) -> Address {
    address.clone()
}

// ─── Structural validation ────────────────────────────────────────────────────

/// Validate that `caller` is structurally eligible to invoke contract operations.
///
/// A caller is valid when:
/// 1. It is **not** the contract's own address (no self-calls).
/// 2. It is **not** on the blacklist.
///
/// This function performs **no** role or permission check — use
/// [`require_caller_role`] when you also need to verify a specific identity.
///
/// # Errors
/// - [`Error::InvalidAddress`] — caller is the contract itself.
/// - [`Error::Unauthorized`]   — caller is blacklisted.
pub fn validate_caller(env: &Env, caller: &Address) -> Result<(), Error> {
    reject_self_call(env, caller)?;
    reject_blacklisted(env, caller)?;
    Ok(())
}

// ─── Role-based authorization ─────────────────────────────────────────────────

/// Validate caller identity and require that `caller` holds `role`.
///
/// Execution order:
/// 1. [`validate_caller`] — structural gate (self-call + blacklist).
/// 2. Role check — verifies `caller` matches the required [`CallerRole`].
/// 3. `caller.require_auth()` — demands a signed authorization envelope from
///    the Soroban host when the role check passes.
///
/// Returns `Ok(())` only when all three stages pass.
///
/// # Errors
/// - [`Error::InvalidAddress`]  — caller is the contract itself.
/// - [`Error::Unauthorized`]    — caller is blacklisted.
/// - [`Error::NotInitialized`]  — contract has no admin configured
///                                (for [`CallerRole::Admin`] only).
/// - [`Error::TokenNotFound`]   — token does not exist
///                                (for [`CallerRole::Owner`] / [`CallerRole::Creator`]).
/// - [`Error::Unauthorized`]    — caller does not hold the required role.
pub fn require_caller_role(env: &Env, caller: &Address, role: CallerRole) -> Result<(), Error> {
    validate_caller(env, caller)?;
    check_role(env, caller, role)?;
    caller.require_auth();
    Ok(())
}

/// Check whether `caller` holds `role` without requiring authorization.
///
/// Performs the same role resolution as [`require_caller_role`] but does
/// **not** call `require_auth`. Use this for non-authoritative probes (e.g.
/// building UI state, fee routing decisions) where you do not want to
/// consume an auth envelope.
///
/// # Errors
/// Same as [`require_caller_role`] except no auth envelope is consumed.
pub fn caller_has_role(env: &Env, caller: &Address, role: CallerRole) -> Result<bool, Error> {
    validate_caller(env, caller)?;
    match check_role(env, caller, role) {
        Ok(()) => Ok(true),
        Err(Error::Unauthorized) => Ok(false),
        Err(e) => Err(e),
    }
}

// ─── Focused rejection helpers (public for composability) ─────────────────────

/// Reject the call if `caller` is the contract's own address.
///
/// Soroban contracts must never accept themselves as a caller for
/// user-facing operations; such calls indicate a logic error or an attack.
///
/// # Errors
/// - [`Error::InvalidAddress`] when `caller == env.current_contract_address()`.
pub fn reject_self_call(env: &Env, caller: &Address) -> Result<(), Error> {
    if *caller == env.current_contract_address() {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Reject the call if `caller` is on the contract blacklist.
///
/// # Errors
/// - [`Error::Unauthorized`] when `caller` is blacklisted.
pub fn reject_blacklisted(env: &Env, caller: &Address) -> Result<(), Error> {
    if blacklist::is_blacklisted(env, caller) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

// ─── Admin helpers (public for guard integration) ─────────────────────────────

/// Return `true` when `caller` is the contract-level admin or a registered
/// multi-admin, without consuming an auth envelope.
///
/// Returns `false` when no admin is configured (uninitialized contract).
pub fn is_admin_caller(env: &Env, caller: &Address) -> bool {
    // Multi-admin registry (highest priority)
    if administrator_storage::is_admin(env, caller) {
        return true;
    }
    // Contract-level admin
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .map(|admin| *caller == admin)
        .unwrap_or(false)
}

// ─── Internal role resolution ─────────────────────────────────────────────────

/// Resolve whether `caller` satisfies `role`.
///
/// Called after [`validate_caller`] so the self-call and blacklist checks have
/// already passed. Returns `Ok(())` when the role is satisfied, or an
/// appropriate [`Error`] when it is not.
fn check_role(env: &Env, caller: &Address, role: CallerRole) -> Result<(), Error> {
    match role {
        CallerRole::Admin => check_admin_role(env, caller),
        CallerRole::Owner(token_id) => check_owner_role(env, caller, token_id),
        CallerRole::Creator(token_id) => check_creator_role(env, caller, token_id),
        CallerRole::Minter => check_minter_role(env, caller),
        CallerRole::Any => Ok(()),
    }
}

fn check_admin_role(env: &Env, caller: &Address) -> Result<(), Error> {
    // Ensure contract is initialized
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }
    if is_admin_caller(env, caller) {
        return Ok(());
    }
    Err(Error::Unauthorized)
}

fn check_owner_role(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let owner = token_owner_storage::get_owner(env, token_id)?; // propagates TokenNotFound
    if *caller == owner {
        return Ok(());
    }
    Err(Error::Unauthorized)
}

fn check_creator_role(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    let creator = creator_storage::get_creator(env, token_id)?; // propagates TokenNotFound
    if *caller == creator {
        return Ok(());
    }
    Err(Error::Unauthorized)
}

fn check_minter_role(env: &Env, caller: &Address) -> Result<(), Error> {
    if mint_authorization::is_minter(env, caller) {
        return Ok(());
    }
    Err(Error::Unauthorized)
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::blacklist;
    use crate::mint_authorization;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ─────────────────────────────────────────────────────────

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_admin(env: &Env, admin: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::save_owner(env, token_id, owner);
    }

    // ── get_caller ───────────────────────────────────────────────────────────

    #[test]
    fn get_caller_returns_the_provided_address() {
        with_contract(|env| {
            let addr = Address::generate(env);
            assert_eq!(get_caller(env, &addr), addr);
        });
    }

    #[test]
    fn get_caller_is_idempotent_across_repeated_calls() {
        with_contract(|env| {
            let addr = Address::generate(env);
            assert_eq!(get_caller(env, &addr), get_caller(env, &addr));
        });
    }

    // ── reject_self_call ─────────────────────────────────────────────────────

    #[test]
    fn reject_self_call_blocks_contract_address() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                reject_self_call(env, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn reject_self_call_allows_external_address() {
        with_contract(|env| {
            let external = Address::generate(env);
            assert!(reject_self_call(env, &external).is_ok());
        });
    }

    // ── reject_blacklisted ───────────────────────────────────────────────────

    #[test]
    fn reject_blacklisted_blocks_blacklisted_address() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            blacklist::add_wallet(env, &wallet);
            assert_eq!(reject_blacklisted(env, &wallet), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn reject_blacklisted_allows_clean_address() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            assert!(reject_blacklisted(env, &wallet).is_ok());
        });
    }

    #[test]
    fn reject_blacklisted_allows_address_after_removal() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            blacklist::add_wallet(env, &wallet);
            blacklist::remove_wallet(env, &wallet);
            assert!(reject_blacklisted(env, &wallet).is_ok());
        });
    }

    // ── validate_caller ──────────────────────────────────────────────────────

    #[test]
    fn validate_caller_accepts_normal_external_address() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(validate_caller(env, &caller).is_ok());
        });
    }

    #[test]
    fn validate_caller_rejects_contract_self_address() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                validate_caller(env, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn validate_caller_rejects_blacklisted_address() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            blacklist::add_wallet(env, &wallet);
            assert_eq!(validate_caller(env, &wallet), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn validate_caller_self_check_runs_before_blacklist() {
        // Self-call must return InvalidAddress, not Unauthorized, even when
        // the contract address is also blacklisted.
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            blacklist::add_wallet(env, &contract_addr);
            assert_eq!(
                validate_caller(env, &contract_addr),
                Err(Error::InvalidAddress)
            );
        });
    }

    // ── require_caller_role — Admin ──────────────────────────────────────────

    #[test]
    fn require_caller_role_admin_accepts_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            assert!(require_caller_role(env, &admin, CallerRole::Admin).is_ok());
        });
    }

    #[test]
    fn require_caller_role_admin_accepts_registered_multi_admin() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);
            assert!(require_caller_role(env, &multi, CallerRole::Admin).is_ok());
        });
    }

    #[test]
    fn require_caller_role_admin_rejects_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            setup_admin(env, &admin);
            assert_eq!(
                require_caller_role(env, &other, CallerRole::Admin),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_caller_role_admin_rejects_when_not_initialized() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                require_caller_role(env, &caller, CallerRole::Admin),
                Err(Error::NotInitialized)
            );
        });
    }

    #[test]
    fn require_caller_role_admin_rejects_blacklisted_admin() {
        // Even if the caller holds admin status, blacklist gate fires first.
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            blacklist::add_wallet(env, &admin);
            assert_eq!(
                require_caller_role(env, &admin, CallerRole::Admin),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── require_caller_role — Owner ──────────────────────────────────────────

    #[test]
    fn require_caller_role_owner_accepts_token_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(require_caller_role(env, &owner, CallerRole::Owner(1)).is_ok());
        });
    }

    #[test]
    fn require_caller_role_owner_rejects_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            setup_token(env, 1, &owner);
            assert_eq!(
                require_caller_role(env, &other, CallerRole::Owner(1)),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_caller_role_owner_rejects_nonexistent_token() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                require_caller_role(env, &caller, CallerRole::Owner(999)),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── require_caller_role — Creator ────────────────────────────────────────

    #[test]
    fn require_caller_role_creator_accepts_token_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            creator_storage::set_creator(env, 1, &creator);
            assert!(require_caller_role(env, &creator, CallerRole::Creator(1)).is_ok());
        });
    }

    #[test]
    fn require_caller_role_creator_rejects_non_creator() {
        with_contract(|env| {
            let creator = Address::generate(env);
            let other = Address::generate(env);
            creator_storage::set_creator(env, 1, &creator);
            assert_eq!(
                require_caller_role(env, &other, CallerRole::Creator(1)),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_caller_role_creator_rejects_nonexistent_token() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                require_caller_role(env, &caller, CallerRole::Creator(999)),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── require_caller_role — Minter ─────────────────────────────────────────

    #[test]
    fn require_caller_role_minter_accepts_approved_minter() {
        with_contract(|env| {
            let minter = Address::generate(env);
            mint_authorization::set_approved_minter(env, &minter);
            assert!(require_caller_role(env, &minter, CallerRole::Minter).is_ok());
        });
    }

    #[test]
    fn require_caller_role_minter_rejects_unapproved_caller() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                require_caller_role(env, &caller, CallerRole::Minter),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_caller_role_minter_rejects_after_removal() {
        with_contract(|env| {
            let minter = Address::generate(env);
            mint_authorization::set_approved_minter(env, &minter);
            mint_authorization::remove_approved_minter(env, &minter);
            assert_eq!(
                require_caller_role(env, &minter, CallerRole::Minter),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── require_caller_role — Any ────────────────────────────────────────────

    #[test]
    fn require_caller_role_any_accepts_normal_address() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(require_caller_role(env, &caller, CallerRole::Any).is_ok());
        });
    }

    #[test]
    fn require_caller_role_any_still_rejects_blacklisted() {
        with_contract(|env| {
            let caller = Address::generate(env);
            blacklist::add_wallet(env, &caller);
            assert_eq!(
                require_caller_role(env, &caller, CallerRole::Any),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn require_caller_role_any_still_rejects_self_call() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();
            assert_eq!(
                require_caller_role(env, &contract_addr, CallerRole::Any),
                Err(Error::InvalidAddress)
            );
        });
    }

    // ── caller_has_role ──────────────────────────────────────────────────────

    #[test]
    fn caller_has_role_returns_true_for_valid_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            assert!(caller_has_role(env, &admin, CallerRole::Admin).unwrap());
        });
    }

    #[test]
    fn caller_has_role_returns_false_for_non_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            setup_admin(env, &admin);
            assert!(!caller_has_role(env, &other, CallerRole::Admin).unwrap());
        });
    }

    #[test]
    fn caller_has_role_propagates_token_not_found() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                caller_has_role(env, &caller, CallerRole::Owner(999)),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn caller_has_role_returns_false_for_non_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let other = Address::generate(env);
            setup_token(env, 1, &owner);
            assert!(!caller_has_role(env, &other, CallerRole::Owner(1)).unwrap());
        });
    }

    // ── is_admin_caller ──────────────────────────────────────────────────────

    #[test]
    fn is_admin_caller_true_for_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            assert!(is_admin_caller(env, &admin));
        });
    }

    #[test]
    fn is_admin_caller_true_for_multi_admin() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);
            assert!(is_admin_caller(env, &multi));
        });
    }

    #[test]
    fn is_admin_caller_false_for_regular_address() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let other = Address::generate(env);
            setup_admin(env, &admin);
            assert!(!is_admin_caller(env, &other));
        });
    }

    #[test]
    fn is_admin_caller_false_when_uninitialized() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert!(!is_admin_caller(env, &caller));
        });
    }

    // ── Guard composition integration ────────────────────────────────────────

    #[test]
    fn composes_with_sequence_guard_both_pass() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                validate_caller(env, &admin),
                require_caller_role(env, &admin, CallerRole::Admin),
            );
            assert!(result.is_ok());
        });
    }

    #[test]
    fn composes_with_sequence_guard_first_fails() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);
            blacklist::add_wallet(env, &admin);

            let result = crate::guard_composition::sequence(
                validate_caller(env, &admin),
                require_caller_role(env, &admin, CallerRole::Admin),
            );
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }

    #[test]
    fn composes_with_guard_builder_all_pass() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            let result = crate::guard_composition::GuardBuilder::new()
                .add(validate_caller(env, &owner))
                .add(require_caller_role(env, &owner, CallerRole::Owner(1)))
                .execute();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn composes_with_guard_builder_stops_on_first_failure() {
        with_contract(|env| {
            let contract_addr = env.current_contract_address();

            let result = crate::guard_composition::GuardBuilder::new()
                .add(validate_caller(env, &contract_addr))
                .add(Ok(())) // would pass but should never execute
                .execute();
            assert_eq!(result, Err(Error::InvalidAddress));
        });
    }
}
