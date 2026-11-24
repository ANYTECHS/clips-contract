//! Role-Based Access Control (RBAC) — issue #1254.
//!
//! Introduces a unified, contract-wide permission system that maps roles to
//! permitted actions and validates caller authorization before any protected
//! operation executes.
//!
//! # Acceptance criteria
//!
//! | Criterion | Implementation |
//! |-----------|----------------|
//! | Define supported roles | [`ContractRole`] enum |
//! | Associate roles with permitted actions | [`ContractAction`] enum + [`role_permits_action`] |
//! | Validate roles before protected operations | [`require_role`] + [`check_caller_action`] |
//! | Add role authorization tests | `tests` module |
//!
//! # Design
//!
//! The RBAC system is built from three orthogonal primitives:
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`ContractRole`] | Named identity / permission level a caller may hold |
//! | [`ContractAction`] | An operation the contract exposes |
//! | [`RoleContext`] | Optional token-scoped context needed for token-level roles |
//!
//! The public API composes these into two guard functions:
//!
//! - [`require_role`] — resolve what role a caller holds, then enforce
//!   `require_auth`. Returns the resolved role so callers can branch on it.
//! - [`check_caller_action`] — one-shot guard: resolve role → check permission →
//!   require auth. The canonical entry point for protected entry points.
//!
//! # Role hierarchy
//!
//! Roles are resolved in **priority order**. The first match wins.
//!
//! | Priority | Role | Resolution source |
//! |----------|------|-------------------|
//! | 1 | [`ContractRole::Admin`] | Multi-admin registry ([`administrator_storage`]) + `DataKey::Admin` |
//! | 2 | [`ContractRole::Minter`] | `DataKey::ApprovedMinter` ([`mint_authorization`]) |
//! | 3 | [`ContractRole::Creator`] | `DataKey::Creator(token_id)` ([`creator_storage`]) |
//! | 4 | [`ContractRole::Owner`] | `DataKey::TokenOwner(token_id)` ([`token_owner_storage`]) |
//! | 5 | [`ContractRole::User`] | Any structurally valid caller |
//!
//! # Action permission matrix
//!
//! | Action | Admin | Minter | Creator | Owner | User |
//! |--------|:-----:|:------:|:-------:|:-----:|:----:|
//! | `ConfigureContract` | ✓ | | | | |
//! | `PauseContract` | ✓ | | | | |
//! | `MintToken` | ✓ | ✓ | | | |
//! | `BurnToken` | ✓ | | ✓ | ✓ | |
//! | `TransferToken` | ✓ | | ✓ | ✓ | |
//! | `UpdateMetadata` | ✓ | | ✓ | | |
//! | `SetRoyalty` | ✓ | | ✓ | ✓ | |
//! | `FreezeToken` | ✓ | | | | |
//! | `ManageBlacklist` | ✓ | | | | |
//! | `ApproveTransfer` | ✓ | | | ✓ | |
//! | `ViewData` | ✓ | ✓ | ✓ | ✓ | ✓ |
//!
//! # Usage
//!
//! ```rust,ignore
//! // Protect a mint entry point — both Admin and Minter are allowed
//! role_based_access_control::check_caller_action(
//!     &env,
//!     &caller,
//!     ContractAction::MintToken,
//!     None,
//! )?;
//!
//! // Protect a token transfer — Creator, Owner, and Admin are allowed
//! role_based_access_control::check_caller_action(
//!     &env,
//!     &caller,
//!     ContractAction::TransferToken,
//!     Some(RoleContext::Token(token_id)),
//! )?;
//!
//! // Read the caller's role without enforcing a specific action
//! let role = role_based_access_control::require_role(&env, &caller, None)?;
//! ```
//!
//! # Errors
//!
//! | Error | Condition |
//! |-------|-----------|
//! | [`Error::NotInitialized`] | Contract has no admin configured. |
//! | [`Error::Unauthorized`] | Caller does not hold a role that permits the requested action. |

use soroban_sdk::{Address, Env};

use crate::administrator_storage;
use crate::creator_storage;
use crate::mint_authorization;
use crate::token_owner_storage;
use crate::types::{DataKey, Error, TokenId};

// ─── Role definition — AC1 ────────────────────────────────────────────────────

/// All roles recognized by the contract's RBAC system.
///
/// Roles are resolved in priority order (Admin first, User last). The first
/// matching role wins; resolution stops there.
///
/// Each role variant maps to a distinct set of permitted [`ContractAction`]s
/// (see [`role_permits_action`] and the permission matrix in the module docs).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractRole {
    /// Full administrative access — can configure, pause, freeze, and manage
    /// blacklists. May also perform all lower-privilege operations.
    Admin,

    /// May mint new NFTs. Granted by the contract admin via the approved-minter
    /// registry ([`DataKey::ApprovedMinter`]).
    Minter,

    /// Original creator of a specific token. Resolved per-token from
    /// [`DataKey::Creator`]. May update metadata, set royalties, transfer,
    /// and burn their own creations.
    Creator,

    /// Current owner of a specific token. Resolved per-token from
    /// [`DataKey::TokenOwner`]. May transfer, burn, approve, and set royalties
    /// on owned tokens.
    Owner,

    /// Any structurally valid caller that does not hold a higher-privilege role.
    /// Permitted only for read operations ([`ContractAction::ViewData`]).
    User,
}

// ─── Action definition — AC2 (part 1) ────────────────────────────────────────

/// Contract operations protected by the RBAC system.
///
/// Each variant represents a class of mutating or privileged operations. Pass
/// one of these to [`check_caller_action`] to enforce role-based authorization
/// before executing the operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractAction {
    /// Update global contract configuration (admin, fees, royalty cap).
    ConfigureContract,

    /// Pause or unpause the contract.
    PauseContract,

    /// Mint a new NFT.
    MintToken,

    /// Burn (destroy) a token.
    BurnToken,

    /// Transfer a token to a new owner.
    TransferToken,

    /// Update a token's metadata.
    UpdateMetadata,

    /// Set or update the royalty configuration for a token.
    SetRoyalty,

    /// Freeze a token (make it non-transferable).
    FreezeToken,

    /// Add or remove addresses from the contract blacklist.
    ManageBlacklist,

    /// Grant a transfer approval for a specific token or operator.
    ApproveTransfer,

    /// Read contract state (non-mutating view operations).
    ViewData,
}

// ─── Role context ─────────────────────────────────────────────────────────────

/// Optional context supplied when resolving token-scoped roles.
///
/// `Creator` and `Owner` roles are only meaningful in the context of a
/// specific token. Pass [`RoleContext::Token`] to enable these checks.
/// Omit context (pass `None`) when the operation is not token-scoped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoleContext {
    /// The operation applies to a specific token.
    Token(TokenId),
}

// ─── Permission table — AC2 (part 2) ─────────────────────────────────────────

/// Return `true` when `role` is permitted to perform `action`.
///
/// This function is the single source of truth for the role→action permission
/// matrix. Every authorization decision in the contract should route through
/// here rather than encoding its own ad-hoc privilege logic.
///
/// # Permission matrix
///
/// See the module-level documentation table for the complete breakdown.
pub fn role_permits_action(role: &ContractRole, action: &ContractAction) -> bool {
    match role {
        ContractRole::Admin => {
            // Admins may perform any action.
            true
        }

        ContractRole::Minter => matches!(
            action,
            ContractAction::MintToken | ContractAction::ViewData
        ),

        ContractRole::Creator => matches!(
            action,
            ContractAction::BurnToken
                | ContractAction::TransferToken
                | ContractAction::UpdateMetadata
                | ContractAction::SetRoyalty
                | ContractAction::ViewData
        ),

        ContractRole::Owner => matches!(
            action,
            ContractAction::BurnToken
                | ContractAction::TransferToken
                | ContractAction::SetRoyalty
                | ContractAction::ApproveTransfer
                | ContractAction::ViewData
        ),

        ContractRole::User => matches!(action, ContractAction::ViewData),
    }
}

// ─── Role resolution ──────────────────────────────────────────────────────────

/// Resolve the highest-privilege role held by `caller`.
///
/// Roles are checked in priority order: Admin → Minter → Creator → Owner → User.
/// The first matching role is returned; resolution stops there.
///
/// Token-scoped roles (`Creator`, `Owner`) are only checked when
/// `context` contains a [`RoleContext::Token`] value; without one they are
/// skipped.
///
/// # Errors
/// - [`Error::NotInitialized`] — no admin has been configured yet.
pub fn resolve_role(
    env: &Env,
    caller: &Address,
    context: Option<&RoleContext>,
) -> Result<ContractRole, Error> {
    // Contract must be initialized before any role can be meaningful.
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(Error::NotInitialized);
    }

    // Priority 1 — Admin
    if is_admin(env, caller) {
        return Ok(ContractRole::Admin);
    }

    // Priority 2 — Minter
    if mint_authorization::is_minter(env, caller) {
        return Ok(ContractRole::Minter);
    }

    // Priority 3 — Creator (token-scoped)
    if let Some(RoleContext::Token(token_id)) = context {
        if let Ok(creator) = creator_storage::get_creator(env, *token_id) {
            if creator == *caller {
                return Ok(ContractRole::Creator);
            }
        }
    }

    // Priority 4 — Owner (token-scoped)
    if let Some(RoleContext::Token(token_id)) = context {
        if let Ok(owner) = token_owner_storage::get_owner(env, *token_id) {
            if owner == *caller {
                return Ok(ContractRole::Owner);
            }
        }
    }

    // Priority 5 — User (any valid caller)
    Ok(ContractRole::User)
}

// ─── Authorization guards — AC3 ───────────────────────────────────────────────

/// Resolve the caller's role and demand a signed authorization envelope.
///
/// This is a lower-level building block. It resolves the caller's role for the
/// given context and calls `caller.require_auth()`, but does **not** check
/// whether that role is permitted to perform any specific action. Use
/// [`check_caller_action`] for the full role+action guard.
///
/// Returns the resolved [`ContractRole`] so callers can branch on it.
///
/// # Errors
/// - [`Error::NotInitialized`] — no admin has been configured yet.
pub fn require_role(
    env: &Env,
    caller: &Address,
    context: Option<&RoleContext>,
) -> Result<ContractRole, Error> {
    let role = resolve_role(env, caller, context)?;
    caller.require_auth();
    Ok(role)
}

/// Validate that `caller` holds a role permitted to perform `action`.
///
/// This is the **canonical single-call guard** for all protected entry points.
/// It:
///
/// 1. Resolves the caller's role (see [`resolve_role`]).
/// 2. Demands a signed authorization envelope ([`Address::require_auth`]).
/// 3. Checks the resolved role against the action permission table
///    ([`role_permits_action`]).
///
/// Returns the resolved [`ContractRole`] on success so callers can log or
/// branch on the identity without re-reading storage.
///
/// # Errors
/// - [`Error::NotInitialized`] — no admin has been configured.
/// - [`Error::Unauthorized`] — caller's role does not permit `action`.
///
/// # Example
///
/// ```rust,ignore
/// let role = role_based_access_control::check_caller_action(
///     &env,
///     &caller,
///     ContractAction::MintToken,
///     None,
/// )?;
/// // role is Admin or Minter (the only roles that may mint)
/// ```
pub fn check_caller_action(
    env: &Env,
    caller: &Address,
    action: ContractAction,
    context: Option<&RoleContext>,
) -> Result<ContractRole, Error> {
    let role = require_role(env, caller, context)?;

    if role_permits_action(&role, &action) {
        return Ok(role);
    }

    Err(Error::Unauthorized)
}

// ─── Read-only role probe ─────────────────────────────────────────────────────

/// Check whether `caller` may perform `action` without demanding a signed
/// authorization envelope.
///
/// This is the non-authoritative counterpart of [`check_caller_action`].  Use
/// it for routing decisions and UI state building where consuming an auth
/// envelope would be wrong.
///
/// Returns `Ok(true)` when permitted, `Ok(false)` when not, and propagates
/// infrastructure errors (e.g. [`Error::NotInitialized`]).
pub fn caller_may_perform(
    env: &Env,
    caller: &Address,
    action: &ContractAction,
    context: Option<&RoleContext>,
) -> Result<bool, Error> {
    let role = resolve_role(env, caller, context)?;
    Ok(role_permits_action(&role, action))
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Return `true` when `caller` is the contract-level admin or a registered
/// multi-admin.
fn is_admin(env: &Env, caller: &Address) -> bool {
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::creator_storage;
    use crate::mint_authorization;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Test harness ──────────────────────────────────────────────────────────

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

    fn setup_token_roles(
        env: &Env,
        token_id: TokenId,
        creator: &Address,
        owner: &Address,
    ) {
        creator_storage::set_creator(env, token_id, creator);
        token_owner_storage::save_owner(env, token_id, owner);
    }

    // ── AC1: role_permits_action — permission matrix ──────────────────────────

    #[test]
    fn admin_may_perform_all_actions() {
        let all_actions = [
            ContractAction::ConfigureContract,
            ContractAction::PauseContract,
            ContractAction::MintToken,
            ContractAction::BurnToken,
            ContractAction::TransferToken,
            ContractAction::UpdateMetadata,
            ContractAction::SetRoyalty,
            ContractAction::FreezeToken,
            ContractAction::ManageBlacklist,
            ContractAction::ApproveTransfer,
            ContractAction::ViewData,
        ];
        for action in &all_actions {
            assert!(
                role_permits_action(&ContractRole::Admin, action),
                "Admin should be permitted to perform {:?}",
                action
            );
        }
    }

    #[test]
    fn minter_may_only_mint_and_view() {
        assert!(role_permits_action(&ContractRole::Minter, &ContractAction::MintToken));
        assert!(role_permits_action(&ContractRole::Minter, &ContractAction::ViewData));

        let denied = [
            ContractAction::ConfigureContract,
            ContractAction::PauseContract,
            ContractAction::BurnToken,
            ContractAction::TransferToken,
            ContractAction::UpdateMetadata,
            ContractAction::SetRoyalty,
            ContractAction::FreezeToken,
            ContractAction::ManageBlacklist,
            ContractAction::ApproveTransfer,
        ];
        for action in &denied {
            assert!(
                !role_permits_action(&ContractRole::Minter, action),
                "Minter should NOT be permitted to perform {:?}",
                action
            );
        }
    }

    #[test]
    fn creator_permitted_actions() {
        let allowed = [
            ContractAction::BurnToken,
            ContractAction::TransferToken,
            ContractAction::UpdateMetadata,
            ContractAction::SetRoyalty,
            ContractAction::ViewData,
        ];
        for action in &allowed {
            assert!(
                role_permits_action(&ContractRole::Creator, action),
                "Creator should be permitted to perform {:?}",
                action
            );
        }

        let denied = [
            ContractAction::ConfigureContract,
            ContractAction::PauseContract,
            ContractAction::MintToken,
            ContractAction::FreezeToken,
            ContractAction::ManageBlacklist,
            ContractAction::ApproveTransfer,
        ];
        for action in &denied {
            assert!(
                !role_permits_action(&ContractRole::Creator, action),
                "Creator should NOT be permitted to perform {:?}",
                action
            );
        }
    }

    #[test]
    fn owner_permitted_actions() {
        let allowed = [
            ContractAction::BurnToken,
            ContractAction::TransferToken,
            ContractAction::SetRoyalty,
            ContractAction::ApproveTransfer,
            ContractAction::ViewData,
        ];
        for action in &allowed {
            assert!(
                role_permits_action(&ContractRole::Owner, action),
                "Owner should be permitted to perform {:?}",
                action
            );
        }

        let denied = [
            ContractAction::ConfigureContract,
            ContractAction::PauseContract,
            ContractAction::MintToken,
            ContractAction::UpdateMetadata,
            ContractAction::FreezeToken,
            ContractAction::ManageBlacklist,
        ];
        for action in &denied {
            assert!(
                !role_permits_action(&ContractRole::Owner, action),
                "Owner should NOT be permitted to perform {:?}",
                action
            );
        }
    }

    #[test]
    fn user_may_only_view() {
        assert!(role_permits_action(&ContractRole::User, &ContractAction::ViewData));

        let denied = [
            ContractAction::ConfigureContract,
            ContractAction::PauseContract,
            ContractAction::MintToken,
            ContractAction::BurnToken,
            ContractAction::TransferToken,
            ContractAction::UpdateMetadata,
            ContractAction::SetRoyalty,
            ContractAction::FreezeToken,
            ContractAction::ManageBlacklist,
            ContractAction::ApproveTransfer,
        ];
        for action in &denied {
            assert!(
                !role_permits_action(&ContractRole::User, action),
                "User should NOT be permitted to perform {:?}",
                action
            );
        }
    }

    // ── AC2: resolve_role — role resolution priority order ────────────────────

    #[test]
    fn resolve_role_returns_admin_for_contract_level_admin() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            assert_eq!(resolve_role(env, &admin, None), Ok(ContractRole::Admin));
        });
    }

    #[test]
    fn resolve_role_returns_admin_for_multi_admin() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);

            assert_eq!(resolve_role(env, &multi, None), Ok(ContractRole::Admin));
        });
    }

    #[test]
    fn resolve_role_returns_minter_for_approved_minter() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            assert_eq!(resolve_role(env, &minter, None), Ok(ContractRole::Minter));
        });
    }

    #[test]
    fn resolve_role_returns_creator_for_token_creator() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            assert_eq!(
                resolve_role(env, &creator, Some(&RoleContext::Token(1))),
                Ok(ContractRole::Creator)
            );
        });
    }

    #[test]
    fn resolve_role_returns_owner_for_token_owner() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            assert_eq!(
                resolve_role(env, &owner, Some(&RoleContext::Token(1))),
                Ok(ContractRole::Owner)
            );
        });
    }

    #[test]
    fn resolve_role_returns_user_for_unknown_address() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);

            assert_eq!(resolve_role(env, &stranger, None), Ok(ContractRole::User));
        });
    }

    #[test]
    fn resolve_role_admin_takes_priority_over_minter() {
        // An address registered both as multi-admin and approved-minter
        // should resolve to Admin (higher priority).
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi_admin = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi_admin);
            mint_authorization::set_approved_minter(env, &multi_admin);

            assert_eq!(
                resolve_role(env, &multi_admin, None),
                Ok(ContractRole::Admin)
            );
        });
    }

    #[test]
    fn resolve_role_minter_takes_priority_over_creator() {
        // An address that is both an approved minter and a token creator
        // should resolve to Minter (higher priority).
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter_creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter_creator);
            setup_token_roles(env, 1, &minter_creator, &owner);

            assert_eq!(
                resolve_role(env, &minter_creator, Some(&RoleContext::Token(1))),
                Ok(ContractRole::Minter)
            );
        });
    }

    #[test]
    fn resolve_role_creator_takes_priority_over_owner_for_same_address() {
        // When the creator and owner are the same address, Creator wins
        // (higher priority).
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator_owner = Address::generate(env);
            setup_admin(env, &admin);
            // Same address is both creator and owner
            setup_token_roles(env, 1, &creator_owner, &creator_owner);

            assert_eq!(
                resolve_role(env, &creator_owner, Some(&RoleContext::Token(1))),
                Ok(ContractRole::Creator)
            );
        });
    }

    #[test]
    fn resolve_role_skips_token_roles_without_context() {
        // Without a token context, Creator and Owner checks are skipped;
        // the address should fall through to User.
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            // No context — should be User, not Creator or Owner
            assert_eq!(resolve_role(env, &creator, None), Ok(ContractRole::User));
            assert_eq!(resolve_role(env, &owner, None), Ok(ContractRole::User));
        });
    }

    #[test]
    fn resolve_role_returns_not_initialized_when_no_admin() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(resolve_role(env, &caller, None), Err(Error::NotInitialized));
        });
    }

    // ── AC3: check_caller_action — protected operation guard ─────────────────

    #[test]
    fn check_caller_action_allows_admin_to_configure_contract() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = check_caller_action(env, &admin, ContractAction::ConfigureContract, None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), ContractRole::Admin);
        });
    }

    #[test]
    fn check_caller_action_allows_minter_to_mint() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            let result = check_caller_action(env, &minter, ContractAction::MintToken, None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), ContractRole::Minter);
        });
    }

    #[test]
    fn check_caller_action_allows_creator_to_update_metadata() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            let result = check_caller_action(
                env,
                &creator,
                ContractAction::UpdateMetadata,
                Some(&RoleContext::Token(1)),
            );
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), ContractRole::Creator);
        });
    }

    #[test]
    fn check_caller_action_allows_owner_to_transfer() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            let result = check_caller_action(
                env,
                &owner,
                ContractAction::TransferToken,
                Some(&RoleContext::Token(1)),
            );
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), ContractRole::Owner);
        });
    }

    #[test]
    fn check_caller_action_allows_user_to_view_data() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);

            let result = check_caller_action(env, &stranger, ContractAction::ViewData, None);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), ContractRole::User);
        });
    }

    #[test]
    fn check_caller_action_rejects_minter_trying_to_configure() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            assert_eq!(
                check_caller_action(env, &minter, ContractAction::ConfigureContract, None),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn check_caller_action_rejects_owner_trying_to_update_metadata() {
        // Owner is not permitted to update metadata — only Creator or Admin may.
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            assert_eq!(
                check_caller_action(
                    env,
                    &owner,
                    ContractAction::UpdateMetadata,
                    Some(&RoleContext::Token(1))
                ),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn check_caller_action_rejects_user_trying_to_mint() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);

            assert_eq!(
                check_caller_action(env, &stranger, ContractAction::MintToken, None),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn check_caller_action_rejects_creator_trying_to_freeze() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let creator = Address::generate(env);
            let owner = Address::generate(env);
            setup_admin(env, &admin);
            setup_token_roles(env, 1, &creator, &owner);

            assert_eq!(
                check_caller_action(
                    env,
                    &creator,
                    ContractAction::FreezeToken,
                    Some(&RoleContext::Token(1))
                ),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn check_caller_action_returns_not_initialized_when_no_admin() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                check_caller_action(env, &caller, ContractAction::ViewData, None),
                Err(Error::NotInitialized)
            );
        });
    }

    // ── AC3: caller_may_perform — non-authoritative probe ─────────────────────

    #[test]
    fn caller_may_perform_returns_true_for_permitted_action() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            assert!(
                caller_may_perform(env, &minter, &ContractAction::MintToken, None).unwrap()
            );
        });
    }

    #[test]
    fn caller_may_perform_returns_false_for_denied_action() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            assert!(
                !caller_may_perform(env, &minter, &ContractAction::PauseContract, None).unwrap()
            );
        });
    }

    #[test]
    fn caller_may_perform_returns_false_for_user_trying_to_mint() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);

            assert!(
                !caller_may_perform(env, &stranger, &ContractAction::MintToken, None).unwrap()
            );
        });
    }

    #[test]
    fn caller_may_perform_propagates_not_initialized() {
        with_contract(|env| {
            let caller = Address::generate(env);
            assert_eq!(
                caller_may_perform(env, &caller, &ContractAction::ViewData, None),
                Err(Error::NotInitialized)
            );
        });
    }

    // ── AC4: require_role ─────────────────────────────────────────────────────

    #[test]
    fn require_role_returns_resolved_role_and_requires_auth() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let role = require_role(env, &admin, None).expect("admin should resolve");
            assert_eq!(role, ContractRole::Admin);
        });
    }

    #[test]
    fn require_role_returns_user_for_unknown_caller() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &admin);

            let role = require_role(env, &stranger, None).expect("stranger resolves to User");
            assert_eq!(role, ContractRole::User);
        });
    }

    // ── Guard composition ─────────────────────────────────────────────────────

    #[test]
    fn check_caller_action_composes_with_sequence_guard() {
        with_contract(|env| {
            let admin = Address::generate(env);
            setup_admin(env, &admin);

            let result = crate::guard_composition::sequence(
                check_caller_action(env, &admin, ContractAction::PauseContract, None)
                    .map(|_| ()),
                Ok(()),
            );
            assert!(result.is_ok());
        });
    }

    #[test]
    fn check_caller_action_failure_halts_sequence() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            let result = crate::guard_composition::sequence(
                check_caller_action(env, &minter, ContractAction::ManageBlacklist, None)
                    .map(|_| ()),
                Ok(()),
            );
            assert_eq!(result, Err(Error::Unauthorized));
        });
    }

    // ── Role revocation ───────────────────────────────────────────────────────

    #[test]
    fn minter_role_revoked_on_removal() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            // Granted
            assert_eq!(resolve_role(env, &minter, None), Ok(ContractRole::Minter));

            // Revoked
            mint_authorization::remove_approved_minter(env, &minter);
            assert_eq!(resolve_role(env, &minter, None), Ok(ContractRole::User));
        });
    }

    #[test]
    fn multi_admin_role_revoked_on_removal() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);

            // Granted
            assert_eq!(resolve_role(env, &multi, None), Ok(ContractRole::Admin));

            // Revoked
            administrator_storage::remove_admin(env, &multi);
            assert_eq!(resolve_role(env, &multi, None), Ok(ContractRole::User));
        });
    }

    // ── Determinism ───────────────────────────────────────────────────────────

    #[test]
    fn resolve_role_is_deterministic() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let minter = Address::generate(env);
            setup_admin(env, &admin);
            mint_authorization::set_approved_minter(env, &minter);

            for _ in 0..3 {
                assert_eq!(resolve_role(env, &admin, None), Ok(ContractRole::Admin));
                assert_eq!(resolve_role(env, &minter, None), Ok(ContractRole::Minter));
            }
        });
    }
}
