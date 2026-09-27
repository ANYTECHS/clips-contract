//! Royalty Authorization Guard (issues #1028, #1072).
//!
//! A **unified, composable guard** that enforces every pre-condition required
//! before a sensitive royalty configuration change is permitted. Rather than
//! calling multiple guards independently (and risking that one is forgotten),
//! callers invoke [`require_royalty_auth`] once and receive a single, ordered
//! rejection surface.
//!
//! Issue #1072 integrates this guard with the royalty state lifecycle: the
//! guard validates the authorized caller, prevents unauthorized royalty
//! changes, and enforces royalty state (existence + frozen checks) before any
//! mutation. [`crate::royalty_updater::update_royalty_configuration`] funnels
//! through this guard so sensitive royalty configuration changes are
//! restricted to authorized accounts.
//!
//! # Authorization model
//!
//! A caller is considered authorized when **all** of the following hold:
//!
//! 1. **Contract is not paused** — [`crate::royalty_pause_guard`] rejects any
//!    call made while the contract is paused.
//! 2. **Royalty state is active** — the token must exist and its royalty
//!    configuration must not be frozen ([`crate::royalty_lifecycle`]).
//! 3. **Caller identity** — the caller must be one of:
//!    - a registered administrator ([`crate::administrator_storage`]), **or**
//!    - the contract-level admin stored at `DataKey::Admin`, **or**
//!    - the token creator ([`crate::creator_storage`]), **or**
//!    - the token owner ([`crate::token_owner_storage`]).
//!
//!    `require_auth` is called for the matched identity before the check
//!    succeeds, so the Soroban host enforces the cryptographic signature.
//!
//! # Usage
//!
//! ```rust,ignore
//! royalty_auth_guard::require_royalty_auth(env, &caller, token_id)?;
//! ```
//!
//! # Errors (in evaluation order)
//!
//! | Error | Condition |
//! |---|---|
//! | [`Error::ContractPaused`] | Contract is currently paused. |
//! | [`Error::TokenNotFound`] | Token has no royalty configuration. |
//! | [`Error::RoyaltyFrozen`] | Royalty configuration is permanently frozen. |
//! | [`Error::UnauthorizedConfigurationUpdate`] | Caller matches no authorized identity. |

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::royalty_lifecycle::validate_state_for_update;
use crate::royalty_pause_guard::require_royalty_not_paused;
use crate::types::{DataKey, Error, TokenId};

// ─── Primary entry point ────────────────────────────────────────────────────

/// Enforce all royalty-configuration pre-conditions for `caller` on `token_id`.
///
/// Checks, in order:
/// 1. Contract is not paused ([`Error::ContractPaused`]).
/// 2. Token exists and royalty is not frozen
///    ([`Error::TokenNotFound`] / [`Error::RoyaltyFrozen`]).
/// 3. Caller is an authorized identity ([`Error::UnauthorizedConfigurationUpdate`]).
///
/// `require_auth` is called for the matched identity, so the Soroban host
/// enforces the cryptographic signature before this function returns `Ok(())`.
///
/// # Errors
/// - [`Error::ContractPaused`]                   — contract is paused.
/// - [`Error::TokenNotFound`]                     — token has no royalty config.
/// - [`Error::RoyaltyFrozen`]                     — royalty is permanently frozen.
/// - [`Error::UnauthorizedConfigurationUpdate`]   — caller not authorized.
pub fn require_royalty_auth(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    // Stage 1 — operational guard.
    require_royalty_not_paused(env)?;
    // Stage 2 — royalty lifecycle state.
    validate_state_for_update(env, token_id)?;
    // Stage 3 — caller authorization.
    authorize_caller(env, caller, token_id)
}

// ─── Admin-only variant ──────────────────────────────────────────────────────

/// Enforce royalty-configuration pre-conditions for administrators only.
///
/// Identical to [`require_royalty_auth`] except that the caller **must** be a
/// registered administrator (via [`administrator_storage`]) or the contract-level
/// admin. Token creator and owner identities are **not** accepted. Use this
/// variant for privileged operations (e.g. emergency overrides, batch freezes)
/// that should be restricted to platform-level operators.
///
/// # Errors
/// - [`Error::ContractPaused`]                   — contract is paused.
/// - [`Error::TokenNotFound`]                     — token has no royalty config.
/// - [`Error::RoyaltyFrozen`]                     — royalty is permanently frozen.
/// - [`Error::UnauthorizedConfigurationUpdate`]   — caller is not an admin.
pub fn require_royalty_admin_auth(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    require_royalty_not_paused(env)?;
    validate_state_for_update(env, token_id)?;
    authorize_admin_only(env, caller)
}

// ─── Pre-token (mint-time) variant ───────────────────────────────────────────

/// Check royalty state pre-conditions without requiring an existing token.
///
/// Use when you need to verify that:
/// - the contract is not paused, and
/// - the caller holds a recognized admin identity,
///
/// but **before** a token has been created (e.g. during mint-time royalty
/// initialization where a `token_id` may not yet exist).
///
/// # Errors
/// - [`Error::ContractPaused`]                   — contract is paused.
/// - [`Error::UnauthorizedConfigurationUpdate`]   — caller is not authorized.
pub fn require_royalty_auth_no_token(env: &Env, caller: &Address) -> Result<(), Error> {
    require_royalty_not_paused(env)?;
    authorize_admin_only(env, caller)
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Resolve whether `caller` is authorized to modify a token's royalty config.
///
/// Authorization order (first match wins):
/// 1. Registered multi-admin ([`administrator_storage::is_admin`]).
/// 2. Contract-level admin at `DataKey::Admin`.
/// 3. Token creator ([`crate::creator_storage`]).
/// 4. Token owner ([`crate::token_owner_storage`]).
fn authorize_caller(env: &Env, caller: &Address, token_id: TokenId) -> Result<(), Error> {
    // 1. Registered multi-admin.
    if administrator_storage::is_admin(env, caller) {
        caller.require_auth();
        return Ok(());
    }

    // 2. Contract-level admin.
    if let Some(admin) = env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
    {
        if *caller == admin {
            caller.require_auth();
            return Ok(());
        }
    }

    // 3. Token creator.
    if let Ok(creator) = crate::creator_storage::get_creator(env, token_id) {
        if *caller == creator {
            caller.require_auth();
            return Ok(());
        }
    }

    // 4. Token owner.
    if let Ok(owner) = crate::token_owner_storage::get_owner(env, token_id) {
        if *caller == owner {
            caller.require_auth();
            return Ok(());
        }
    }

    Err(Error::UnauthorizedConfigurationUpdate)
}

/// Resolve whether `caller` is an admin identity (multi-admin registry or
/// contract-level admin). Does **not** accept creators or owners.
fn authorize_admin_only(env: &Env, caller: &Address) -> Result<(), Error> {
    if administrator_storage::is_admin(env, caller) {
        caller.require_auth();
        return Ok(());
    }

    if let Some(admin) = env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
    {
        if *caller == admin {
            caller.require_auth();
            return Ok(());
        }
    }

    Err(Error::UnauthorizedConfigurationUpdate)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::pause_state::save_pause_state;
    use crate::token_storage;
    use crate::types::{Royalty, RoyaltyRecipient};
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ────────────────────────────────────────────────────────

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    const TOKEN: TokenId = 1;

    /// Seed a token with admin / creator / owner identities and an active royalty.
    fn seed_token(
        env: &Env,
        token_id: TokenId,
        admin: &Address,
        creator: &Address,
        owner: &Address,
    ) {
        env.storage().instance().set(&DataKey::Admin, admin);
        crate::creator_storage::set_creator(env, token_id, creator);
        crate::token_owner_storage::save_owner(env, token_id, owner);
        token_storage::set_royalty(
            env,
            token_id,
            &Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: Address::generate(env),
                        basis_points: 500,
                    }
                ],
                asset_address: None,
            },
        );
    }

    // ── require_royalty_auth — authorized callers ───────────────────────────

    #[test]
    fn contract_admin_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            assert!(require_royalty_auth(env, &admin, TOKEN).is_ok());
        });
    }

    #[test]
    fn creator_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            assert!(require_royalty_auth(env, &creator, TOKEN).is_ok());
        });
    }

    #[test]
    fn owner_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            assert!(require_royalty_auth(env, &owner, TOKEN).is_ok());
        });
    }

    #[test]
    fn registered_multi_admin_is_authorized() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let multi_admin = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(require_royalty_auth(env, &multi_admin, TOKEN).is_ok());
        });
    }

    // ── require_royalty_auth — unauthorized caller ──────────────────────────

    #[test]
    fn unauthorized_caller_is_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let interloper = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            assert_eq!(
                require_royalty_auth(env, &interloper, TOKEN),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    #[test]
    fn revoked_multi_admin_is_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let multi_admin = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            administrator_storage::add_admin(env, &multi_admin);
            administrator_storage::remove_admin(env, &multi_admin);

            assert_eq!(
                require_royalty_auth(env, &multi_admin, TOKEN),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    // ── require_royalty_auth — contract-paused guard ────────────────────────

    #[test]
    fn paused_contract_blocks_all_callers() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            save_pause_state(env, true);

            assert_eq!(
                require_royalty_auth(env, &admin, TOKEN),
                Err(Error::ContractPaused)
            );
            assert_eq!(
                require_royalty_auth(env, &creator, TOKEN),
                Err(Error::ContractPaused)
            );
            assert_eq!(
                require_royalty_auth(env, &owner, TOKEN),
                Err(Error::ContractPaused)
            );
        });
    }

    #[test]
    fn unpaused_contract_allows_authorized_caller() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            save_pause_state(env, true);
            save_pause_state(env, false);

            assert!(require_royalty_auth(env, &admin, TOKEN).is_ok());
        });
    }

    // ── require_royalty_auth — royalty-frozen guard ─────────────────────────

    #[test]
    fn frozen_royalty_blocks_all_callers() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            // Permanently freeze the royalty configuration.
            env.storage()
                .persistent()
                .set(&DataKey::RoyaltyFrozen(TOKEN), &true);

            assert_eq!(
                require_royalty_auth(env, &admin, TOKEN),
                Err(Error::RoyaltyFrozen)
            );
            assert_eq!(
                require_royalty_auth(env, &creator, TOKEN),
                Err(Error::RoyaltyFrozen)
            );
        });
    }

    // ── require_royalty_auth — missing token ────────────────────────────────

    #[test]
    fn missing_token_returns_not_found() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert_eq!(
                require_royalty_auth(env, &admin, 999),
                Err(Error::TokenNotFound)
            );
        });
    }

    // ── require_royalty_admin_auth ──────────────────────────────────────────

    #[test]
    fn admin_auth_accepts_contract_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            assert!(require_royalty_admin_auth(env, &admin, TOKEN).is_ok());
        });
    }

    #[test]
    fn admin_auth_accepts_registered_multi_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            let multi_admin = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(require_royalty_admin_auth(env, &multi_admin, TOKEN).is_ok());
        });
    }

    #[test]
    fn admin_auth_rejects_creator() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            // Creator is not an admin — must be rejected.
            assert_eq!(
                require_royalty_admin_auth(env, &creator, TOKEN),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    #[test]
    fn admin_auth_rejects_owner() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);

            // Owner is not an admin — must be rejected.
            assert_eq!(
                require_royalty_admin_auth(env, &owner, TOKEN),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    #[test]
    fn admin_auth_paused_blocks_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            save_pause_state(env, true);

            assert_eq!(
                require_royalty_admin_auth(env, &admin, TOKEN),
                Err(Error::ContractPaused)
            );
        });
    }

    #[test]
    fn admin_auth_frozen_blocks_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            seed_token(env, TOKEN, &admin, &creator, &owner);
            env.storage()
                .persistent()
                .set(&DataKey::RoyaltyFrozen(TOKEN), &true);

            assert_eq!(
                require_royalty_admin_auth(env, &admin, TOKEN),
                Err(Error::RoyaltyFrozen)
            );
        });
    }

    // ── require_royalty_auth_no_token ────────────────────────────────────────

    #[test]
    fn no_token_auth_accepts_contract_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);

            assert!(require_royalty_auth_no_token(env, &admin).is_ok());
        });
    }

    #[test]
    fn no_token_auth_accepts_registered_multi_admin() {
        with_contract(|env| {
            let multi_admin = Address::generate(env);
            administrator_storage::add_admin(env, &multi_admin);

            assert!(require_royalty_auth_no_token(env, &multi_admin).is_ok());
        });
    }

    #[test]
    fn no_token_auth_rejects_unknown_caller() {
        with_contract(|env| {
            let caller = Address::generate(env);

            assert_eq!(
                require_royalty_auth_no_token(env, &caller),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    #[test]
    fn no_token_auth_paused_blocks_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);
            save_pause_state(env, true);

            assert_eq!(
                require_royalty_auth_no_token(env, &admin),
                Err(Error::ContractPaused)
            );
        });
    }
}
