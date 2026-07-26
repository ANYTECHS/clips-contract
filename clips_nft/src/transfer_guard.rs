//! Transfer authorization guard — validates all pre-conditions before an NFT
//! ownership transfer is executed.
//!
//! This module resolves four related transfer-validation issues:
//!
//! | Issue | Description |
//! |-------|-------------|
//! | [#727] | Validate frozen token status — block transfers of frozen NFTs. |
//! | [#728] | Validate blacklisted wallets — block transfers where sender or recipient is blacklisted. |
//! | [#730] | Transfer authorization guard — allow only owner, approved operator, or admin. |
//! | [#731] | Validate approved operators — verify operator approval before allowing transfers. |
//!
//! # Usage
//!
//! Call [`check_transfer`] at the very start of any transfer entry point.
//! It runs every guard in order and returns the first error encountered.
//!
//! ```rust,ignore
//! transfer_guard::check_transfer(&env, caller, from, to, token_id)?;
//! ```
//!
//! # Guard order
//!
//! 1. Token must exist and `from` must be its owner.
//! 2. Token must **not** be frozen (#727).
//! 3. Neither `from` nor `to` may be blacklisted (#728).
//! 4. `caller` must be the owner, an approved single-token operator, an
//!    approved-for-all operator, or the contract admin (#730 / #731).

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::frozen_token;
use crate::operator_approval;
use crate::owner_storage;
use crate::token_approval;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

// ─── Primary entry point ──────────────────────────────────────────────────────

/// Run all transfer pre-condition checks.
///
/// # Arguments
/// * `env`      — Contract environment.
/// * `caller`   — The address invoking the transfer (must auth).
/// * `from`     — Current owner of the token being transferred.
/// * `to`       — Destination address that will receive ownership.
/// * `token_id` — On-chain identifier of the token to transfer.
///
/// # Errors
/// | Error | Triggered when |
/// |-------|---------------|
/// | `TokenNotFound` | Token does not exist or `from` is not its owner. |
/// | `Unauthorized`  | Token is frozen (#727) or caller is not authorized (#730/#731). |
/// | `InvalidAddress` | Sender or recipient is blacklisted (#728). |
pub fn check_transfer(
    env: &Env,
    caller: &Address,
    from: &Address,
    to: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // 1. Verify the token exists and `from` is the current owner.
    token_owner_storage::verify_owner(env, token_id, from)?;

    // 2. Issue #727 — block transfer if token is frozen.
    check_not_frozen(env, token_id)?;

    // 3. Issue #728 — block transfer if either wallet is blacklisted.
    check_not_blacklisted(env, from, to)?;

    // 4. Issues #730 / #731 — verify caller is authorized to transfer.
    check_caller_authorized(env, caller, from, token_id)?;

    Ok(())
}

// ─── Individual guards ────────────────────────────────────────────────────────

/// Issue #727 — ensure the token is not frozen (soulbound).
///
/// A frozen token is permanently non-transferable.  This check must run
/// before any ownership mutation.
///
/// # Errors
/// - [`Error::Unauthorized`] — token is currently frozen.
pub fn check_not_frozen(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Issue #728 — ensure neither the sender nor recipient is blacklisted.
///
/// If either address has been added to the contract blacklist, the transfer
/// is rejected before any state is changed.
///
/// # Errors
/// - [`Error::InvalidAddress`] — `from` or `to` is on the blacklist.
pub fn check_not_blacklisted(env: &Env, from: &Address, to: &Address) -> Result<(), Error> {
    if blacklist::is_blacklisted(env, from) {
        return Err(Error::InvalidAddress);
    }
    if blacklist::is_blacklisted(env, to) {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

/// Issues #730 / #731 — verify `caller` is permitted to transfer `token_id`.
///
/// A caller is authorized if they are **any** of the following:
/// 1. The token's current owner (`from`).
/// 2. An address with a single-token approval for `token_id` (ERC-721
///    `approve` analogue) — resolves issue #731.
/// 3. An operator approved for **all** of `from`'s tokens (`setApprovalForAll`
///    analogue) — resolves issues #730 and #731.
/// 4. The contract administrator (emergency admin override) — resolves #730.
///
/// # Errors
/// - [`Error::Unauthorized`] — `caller` does not satisfy any of the above.
pub fn check_caller_authorized(
    env: &Env,
    caller: &Address,
    from: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    // 0. Issue #725: Validate Sender Address
    caller.require_auth();

    // 1. Owner may always transfer their own token.
    if caller == from {
        return Ok(());
    }

    // 2. Single-token approval (issue #731).
    if let Some(approved) = token_approval::get_approval(env, token_id) {
        if &approved == caller {
            return Ok(());
        }
    }

    // 3. Operator approved for all tokens of `from` (issues #730 / #731).
    if operator_approval::is_operator(env, from, caller) {
        return Ok(());
    }

    // 4. Contract admin override (issue #730).
    if let Ok(admin) = owner_storage::get_owner(env) {
        if caller == &admin {
            return Ok(());
        }
    }

    Err(Error::Unauthorized)
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blacklist;
    use crate::frozen_token;
    use crate::operator_approval;
    use crate::owner_storage;
    use crate::token_approval;
    use crate::token_owner_storage;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    /// Helper — register the contract and run `f` in its context.
    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    /// Mint a minimal token owned by `owner` for testing purposes.
    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
    }

    // ── Issue #727: check_not_frozen ──────────────────────────────────────────

    #[test]
    fn transfer_allowed_when_token_not_frozen() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            // Token is not frozen — guard must pass.
            assert!(check_not_frozen(env, 1).is_ok());
        });
    }

    #[test]
    fn transfer_blocked_when_token_is_frozen() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);

            assert_eq!(check_not_frozen(env, 1), Err(Error::Unauthorized));
        });
    }

    #[test]
    fn transfer_allowed_after_unfreeze() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);
            frozen_token::unfreeze_token(env, 1);

            assert!(check_not_frozen(env, 1).is_ok());
        });
    }

    // ── Issue #728: check_not_blacklisted ─────────────────────────────────────

    #[test]
    fn transfer_allowed_when_neither_address_blacklisted() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);

            assert!(check_not_blacklisted(env, &from, &to).is_ok());
        });
    }

    #[test]
    fn transfer_blocked_when_sender_blacklisted() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            blacklist::add_wallet(env, &from);

            assert_eq!(
                check_not_blacklisted(env, &from, &to),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn transfer_blocked_when_recipient_blacklisted() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            blacklist::add_wallet(env, &to);

            assert_eq!(
                check_not_blacklisted(env, &from, &to),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn transfer_blocked_when_both_addresses_blacklisted() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            blacklist::add_wallet(env, &from);
            blacklist::add_wallet(env, &to);

            assert_eq!(
                check_not_blacklisted(env, &from, &to),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn transfer_allowed_after_wallet_removed_from_blacklist() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            blacklist::add_wallet(env, &from);
            blacklist::remove_wallet(env, &from);

            assert!(check_not_blacklisted(env, &from, &to).is_ok());
        });
    }

    // ── Issue #730: check_caller_authorized (authorization guard) ─────────────

    #[test]
    fn owner_is_authorized_to_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            assert!(check_caller_authorized(env, &owner, &owner, 1).is_ok());
            
            // Verify that require_auth was invoked
            let auths = env.auths();
            assert!(auths.len() > 0);
            assert_eq!(auths.get_unchecked(0).0, owner);
        });
    }

    #[test]
    fn unauthorized_caller_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);

            assert_eq!(
                check_caller_authorized(env, &stranger, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn admin_is_authorized_to_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let admin = Address::generate(env);
            setup_token(env, 1, &owner);
            owner_storage::save_owner(env, &admin);

            assert!(check_caller_authorized(env, &admin, &owner, 1).is_ok());
        });
    }

    // ── Issue #731: approved operators ───────────────────────────────────────

    #[test]
    fn single_token_approved_address_can_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);

            assert!(check_caller_authorized(env, &approved, &owner, 1).is_ok());
        });
    }

    #[test]
    fn expired_approval_does_not_authorize() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let approved = Address::generate(env);
            setup_token(env, 1, &owner);
            token_approval::save_approval(env, 1, &approved);
            token_approval::remove_approval(env, 1);

            assert_eq!(
                check_caller_authorized(env, &approved, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn operator_approved_for_all_tokens_can_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(check_caller_authorized(env, &operator, &owner, 1).is_ok());
        });
    }

    #[test]
    fn revoked_operator_cannot_transfer() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);
            operator_approval::remove_operator(env, &owner, &operator);

            assert_eq!(
                check_caller_authorized(env, &operator, &owner, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn operator_for_different_owner_cannot_transfer() {
        with_contract(|env| {
            let owner_a = Address::generate(env);
            let owner_b = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner_a);
            // Approve operator only for owner_b, not owner_a.
            operator_approval::save_operator(env, &owner_b, &operator);

            assert_eq!(
                check_caller_authorized(env, &operator, &owner_a, 1),
                Err(Error::Unauthorized)
            );
        });
    }

    // ── check_transfer integration: all guards together ───────────────────────

    #[test]
    fn full_check_passes_for_valid_transfer_by_owner() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);

            assert!(check_transfer(env, &owner, &owner, &Address::generate(env), 1).is_ok());
        });
    }

    #[test]
    fn full_check_fails_when_token_not_owned_by_from() {
        with_contract(|env| {
            let real_owner = Address::generate(env);
            let fake_from = Address::generate(env);
            setup_token(env, 1, &real_owner);

            assert_eq!(
                check_transfer(env, &fake_from, &fake_from, &Address::generate(env), 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn full_check_fails_when_token_frozen() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            frozen_token::freeze_token(env, 1);

            assert_eq!(
                check_transfer(env, &owner, &owner, &Address::generate(env), 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn full_check_fails_when_sender_blacklisted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            setup_token(env, 1, &owner);
            blacklist::add_wallet(env, &owner);

            assert_eq!(
                check_transfer(env, &owner, &owner, &Address::generate(env), 1),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn full_check_fails_when_recipient_blacklisted() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let to = Address::generate(env);
            setup_token(env, 1, &owner);
            blacklist::add_wallet(env, &to);

            assert_eq!(
                check_transfer(env, &owner, &owner, &to, 1),
                Err(Error::InvalidAddress)
            );
        });
    }

    #[test]
    fn full_check_fails_for_unauthorized_caller() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let stranger = Address::generate(env);
            setup_token(env, 1, &owner);

            assert_eq!(
                check_transfer(env, &stranger, &owner, &Address::generate(env), 1),
                Err(Error::Unauthorized)
            );
        });
    }

    #[test]
    fn full_check_passes_for_approved_operator() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 1, &owner);
            operator_approval::save_operator(env, &owner, &operator);

            assert!(
                check_transfer(env, &operator, &owner, &Address::generate(env), 1).is_ok()
            );
        });
    }
}
