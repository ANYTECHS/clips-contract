//! Centralized Authorization Module.
//!
//! Single source of truth for all authorization logic across smart contract
//! operations.  Rules are organized by **contract feature** so that every
//! call-site has one obvious import path and authorization logic can be
//! maintained or changed in a single place.
//!
//! # Feature domains
//!
//! | Domain       | Sub-module       | Responsibility                                  |
//! |--------------|------------------|-------------------------------------------------|
//! | Admin        | [`admin`]        | Contract-level administrative operations        |
//! | Minting      | [`minting`]      | NFT minting and batch-minting operations        |
//! | Marketplace  | [`marketplace`]  | Listing, offer, and purchase operations         |
//! | Royalty      | [`royalty`]      | Royalty configuration and payment operations    |
//!
//! # Design principles
//!
//! 1. **Centralized** — every authorization decision is routed through this
//!    module rather than scattered across individual operation files.
//! 2. **Organized by feature** — callers import `authorization::admin`,
//!    `authorization::minting`, etc., making the authorization surface clear.
//! 3. **Reusable** — each domain exports primitives (`require_*`, `check_*`,
//!    `is_*`) that can be composed by higher-level flows without duplication.
//! 4. **Fail-fast** — all `require_*` functions return
//!    [`Err(Error::Unauthorized)`][crate::types::Error::Unauthorized] (or
//!    [`Err(Error::NotInitialized)`][crate::types::Error::NotInitialized])
//!    immediately on violation; they never silently pass.
//!
//! # Usage
//!
//! ```rust,ignore
//! use clips_nft::authorization;
//!
//! // Administrative operation
//! authorization::admin::require(&env, &caller)?;
//!
//! // Minting authorization
//! authorization::minting::require(&env, &minter)?;
//!
//! // Marketplace — seller must own the token before listing
//! authorization::marketplace::require_seller(&env, token_id, &caller)?;
//!
//! // Royalty configuration change
//! authorization::royalty::require_royalty_admin(&env, &caller)?;
//! ```
//!
//! # Errors
//!
//! All authorization functions propagate [`crate::types::Error`] values:
//!
//! | Error                 | Meaning                                          |
//! |-----------------------|--------------------------------------------------|
//! | `Error::NotInitialized` | Contract has not been initialized (no admin).  |
//! | `Error::Unauthorized`   | Caller does not hold the required permission.  |

use soroban_sdk::{Address, Env};

use crate::types::{DataKey, Error, TokenId};

// ─── Admin ────────────────────────────────────────────────────────────────────

/// Authorization rules for administrative contract operations.
///
/// Admin authorization follows a two-tier resolution order:
///
/// 1. **Multi-admin registry** — an address is an admin if it appears in the
///    registered administrator list (via [`crate::administrator_storage`]).
/// 2. **Contract-level admin** — the single address stored at
///    [`DataKey::Admin`].
///
/// The first match wins.
pub mod admin {
    use super::*;
    use crate::administrator_storage;

    /// Return the primary contract-level admin address.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — no admin has been set.
    pub fn get(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    /// Return `true` if `caller` holds administrative privileges.
    ///
    /// Performs a read-only check with no authorization side effects.
    pub fn is_admin(env: &Env, caller: &Address) -> bool {
        // Priority 1: registered multi-admin list
        if administrator_storage::is_admin(env, caller) {
            return true;
        }
        // Priority 2: contract-level admin
        env.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .map(|a| *caller == a)
            .unwrap_or(false)
    }

    /// Require that `caller` is an authorized administrator.
    ///
    /// Demands a signed authorization envelope and verifies admin membership.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller is not an authorized admin.
    pub fn require(env: &Env, caller: &Address) -> Result<(), Error> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }
        caller.require_auth();
        if is_admin(env, caller) {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }

    /// Require admin authorization without consuming `caller`'s auth envelope.
    ///
    /// Use when the caller's auth has already been consumed by an outer check
    /// and you only need to verify admin membership.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller is not an authorized admin.
    pub fn require_membership(env: &Env, caller: &Address) -> Result<(), Error> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }
        if is_admin(env, caller) {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }
}

// ─── Minting ─────────────────────────────────────────────────────────────────

/// Authorization rules for NFT minting operations.
///
/// A caller is authorized to mint when **any** of the following holds:
///
/// 1. The caller is a registered admin (see [`admin::is_admin`]).
/// 2. The caller is the explicitly approved minter stored at
///    [`DataKey::ApprovedMinter`].
pub mod minting {
    use super::*;

    /// Return `true` if `minter` is authorized to mint NFTs.
    pub fn is_authorized(env: &Env, minter: &Address) -> bool {
        // Priority 1: admin is always allowed to mint
        if admin::is_admin(env, minter) {
            return true;
        }
        // Priority 2: explicitly approved minter role
        env.storage()
            .instance()
            .get::<_, Address>(&DataKey::ApprovedMinter(minter.clone()))
            .map(|approved| *minter == approved)
            .unwrap_or(false)
    }

    /// Require that `minter` is authorized to mint NFTs.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller lacks mint permission.
    pub fn require(env: &Env, minter: &Address) -> Result<(), Error> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }
        minter.require_auth();
        if is_authorized(env, minter) {
            return Ok(());
        }
        Err(Error::Unauthorized)
    }

    /// Require admin authorization specifically for batch-minting.
    ///
    /// Batch minting is restricted to admins only (more privileged than
    /// single-token minting which also allows the approved minter role).
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller is not an admin.
    pub fn require_batch(env: &Env, caller: &Address) -> Result<(), Error> {
        admin::require(env, caller)
    }
}

// ─── Marketplace ─────────────────────────────────────────────────────────────

/// Authorization rules for marketplace operations.
///
/// Marketplace authorization covers three distinct roles:
///
/// - **Seller** — the current token owner; may list or delist the token.
/// - **Buyer** — any address that has placed an offer; may cancel their own offer.
/// - **Admin** — may intervene in any listing or offer as an override.
pub mod marketplace {
    use super::*;
    use crate::token_owner_storage;

    /// Require that `caller` is the owner of `token_id` (i.e. the seller).
    ///
    /// Used to authorize listing creation, listing modification, and listing
    /// cancellation.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller does not own the token.
    pub fn require_seller(env: &Env, token_id: TokenId, caller: &Address) -> Result<(), Error> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }
        caller.require_auth();

        // Admin override: admins can act as sellers for governance purposes
        if admin::is_admin(env, caller) {
            return Ok(());
        }

        let owner = token_owner_storage::get_owner(env, token_id)
            .map_err(|_| Error::Unauthorized)?;
        if *caller == owner {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }

    /// Require that `caller` is `expected_buyer`.
    ///
    /// Used to authorize offer cancellation and withdrawal.
    ///
    /// # Errors
    /// - [`Error::Unauthorized`] — caller is not the expected buyer.
    pub fn require_buyer(env: &Env, caller: &Address, expected_buyer: &Address) -> Result<(), Error> {
        caller.require_auth();
        if caller == expected_buyer || admin::is_admin(env, caller) {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }

    /// Require that `caller` can accept an offer on `token_id`.
    ///
    /// Only the current token owner (seller) or an admin may accept an offer.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller is neither the owner nor an admin.
    pub fn require_offer_acceptor(
        env: &Env,
        token_id: TokenId,
        caller: &Address,
    ) -> Result<(), Error> {
        require_seller(env, token_id, caller)
    }

    /// Return `true` if `caller` is authorized to perform any marketplace operation
    /// as an admin override.
    pub fn is_admin_override(env: &Env, caller: &Address) -> bool {
        admin::is_admin(env, caller)
    }
}

// ─── Royalty ─────────────────────────────────────────────────────────────────

/// Authorization rules for royalty configuration and payment operations.
///
/// Royalty authorization distinguishes:
///
/// - **Royalty admin** — full control over royalty config; currently resolved
///   to the contract admin but could be a separate role in future.
/// - **Royalty recipient** — may trigger payment claims for their own accrued
///   earnings but cannot alter configuration.
pub mod royalty {
    use super::*;

    /// Return `true` if `caller` is authorized to modify royalty configuration.
    pub fn is_royalty_admin(env: &Env, caller: &Address) -> bool {
        admin::is_admin(env, caller)
    }

    /// Require that `caller` is authorized to configure royalties.
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller is not a royalty admin.
    pub fn require_royalty_admin(env: &Env, caller: &Address) -> Result<(), Error> {
        admin::require(env, caller)
    }

    /// Require that `caller` is `expected_recipient` or an admin.
    ///
    /// Used to authorize royalty claim / withdrawal operations.
    ///
    /// # Errors
    /// - [`Error::Unauthorized`] — caller is neither the recipient nor an admin.
    pub fn require_recipient_or_admin(
        env: &Env,
        caller: &Address,
        expected_recipient: &Address,
    ) -> Result<(), Error> {
        caller.require_auth();
        if caller == expected_recipient || admin::is_admin(env, caller) {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }

    /// Require that `caller` can freeze royalties (admin only).
    ///
    /// # Errors
    /// - [`Error::NotInitialized`] — contract has no admin configured.
    /// - [`Error::Unauthorized`] — caller is not an admin.
    pub fn require_freeze(env: &Env, caller: &Address) -> Result<(), Error> {
        admin::require(env, caller)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::administrator_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Helpers ───────────────────────────────────────────────────────────────

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

    // ═══════════════════════════════════════════════════════════════════════════
    // admin domain
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn admin_get_returns_configured_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert_eq!(admin::get(env).unwrap(), a);
        });
    }

    #[test]
    fn admin_get_returns_not_initialized_when_not_set() {
        with_contract(|env| {
            assert_eq!(admin::get(env), Err(Error::NotInitialized));
        });
    }

    #[test]
    fn admin_is_admin_true_for_contract_level_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(admin::is_admin(env, &a));
        });
    }

    #[test]
    fn admin_is_admin_true_for_registered_multi_admin() {
        with_contract(|env| {
            let primary = Address::generate(env);
            let multi = Address::generate(env);
            setup_admin(env, &primary);
            administrator_storage::add_admin(env, &multi);
            assert!(admin::is_admin(env, &multi));
        });
    }

    #[test]
    fn admin_is_admin_false_for_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &a);
            assert!(!admin::is_admin(env, &stranger));
        });
    }

    #[test]
    fn admin_require_succeeds_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(admin::require(env, &a).is_ok());
        });
    }

    #[test]
    fn admin_require_rejects_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &a);
            assert_eq!(admin::require(env, &stranger), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn admin_require_returns_not_initialized_when_no_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            assert_eq!(admin::require(env, &a), Err(Error::NotInitialized));
        });
    }

    #[test]
    fn admin_require_membership_succeeds_for_admin_no_double_auth() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            // membership check does NOT call require_auth, so no double-auth
            assert!(admin::require_membership(env, &a).is_ok());
        });
    }

    #[test]
    fn admin_require_membership_rejects_non_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &a);
            assert_eq!(
                admin::require_membership(env, &stranger),
                Err(Error::Unauthorized)
            );
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // minting domain
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn minting_is_authorized_true_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(minting::is_authorized(env, &a));
        });
    }

    #[test]
    fn minting_is_authorized_false_for_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &a);
            assert!(!minting::is_authorized(env, &stranger));
        });
    }

    #[test]
    fn minting_require_succeeds_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(minting::require(env, &a).is_ok());
        });
    }

    #[test]
    fn minting_require_rejects_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &a);
            assert_eq!(minting::require(env, &stranger), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn minting_require_returns_not_initialized_when_no_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            assert_eq!(minting::require(env, &a), Err(Error::NotInitialized));
        });
    }

    #[test]
    fn minting_require_batch_is_admin_only() {
        with_contract(|env| {
            let a = Address::generate(env);
            let stranger = Address::generate(env);
            setup_admin(env, &a);
            assert!(minting::require_batch(env, &a).is_ok());
            assert_eq!(
                minting::require_batch(env, &stranger),
                Err(Error::Unauthorized)
            );
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // marketplace domain
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn marketplace_require_buyer_accepts_matching_buyer() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            let buyer = Address::generate(env);
            assert!(marketplace::require_buyer(env, &buyer, &buyer).is_ok());
        });
    }

    #[test]
    fn marketplace_require_buyer_rejects_wrong_caller() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            let buyer = Address::generate(env);
            let impostor = Address::generate(env);
            assert_eq!(
                marketplace::require_buyer(env, &impostor, &buyer),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn marketplace_require_buyer_accepts_admin_override() {
        with_contract(|env| {
            let admin_addr = Address::generate(env);
            setup_admin(env, &admin_addr);
            let buyer = Address::generate(env);
            // Admin is not the buyer but override should pass
            assert!(marketplace::require_buyer(env, &admin_addr, &buyer).is_ok());
        });
    }

    #[test]
    fn marketplace_is_admin_override_true_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(marketplace::is_admin_override(env, &a));
        });
    }

    #[test]
    fn marketplace_is_admin_override_false_for_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let s = Address::generate(env);
            setup_admin(env, &a);
            assert!(!marketplace::is_admin_override(env, &s));
        });
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // royalty domain
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn royalty_is_royalty_admin_true_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(royalty::is_royalty_admin(env, &a));
        });
    }

    #[test]
    fn royalty_is_royalty_admin_false_for_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let s = Address::generate(env);
            setup_admin(env, &a);
            assert!(!royalty::is_royalty_admin(env, &s));
        });
    }

    #[test]
    fn royalty_require_royalty_admin_succeeds_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(royalty::require_royalty_admin(env, &a).is_ok());
        });
    }

    #[test]
    fn royalty_require_royalty_admin_rejects_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let s = Address::generate(env);
            setup_admin(env, &a);
            assert_eq!(
                royalty::require_royalty_admin(env, &s),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn royalty_require_recipient_or_admin_accepts_recipient() {
        with_contract(|env| {
            let admin_addr = Address::generate(env);
            setup_admin(env, &admin_addr);
            let recipient = Address::generate(env);
            assert!(royalty::require_recipient_or_admin(env, &recipient, &recipient).is_ok());
        });
    }

    #[test]
    fn royalty_require_recipient_or_admin_accepts_admin() {
        with_contract(|env| {
            let admin_addr = Address::generate(env);
            setup_admin(env, &admin_addr);
            let recipient = Address::generate(env);
            assert!(royalty::require_recipient_or_admin(env, &admin_addr, &recipient).is_ok());
        });
    }

    #[test]
    fn royalty_require_recipient_or_admin_rejects_stranger() {
        with_contract(|env| {
            let admin_addr = Address::generate(env);
            setup_admin(env, &admin_addr);
            let recipient = Address::generate(env);
            let stranger = Address::generate(env);
            assert_eq!(
                royalty::require_recipient_or_admin(env, &stranger, &recipient),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn royalty_require_freeze_succeeds_for_admin() {
        with_contract(|env| {
            let a = Address::generate(env);
            setup_admin(env, &a);
            assert!(royalty::require_freeze(env, &a).is_ok());
        });
    }

    #[test]
    fn royalty_require_freeze_rejects_stranger() {
        with_contract(|env| {
            let a = Address::generate(env);
            let s = Address::generate(env);
            setup_admin(env, &a);
            assert_eq!(royalty::require_freeze(env, &s), Err(Error::Unauthorized));
        });
    }
}
