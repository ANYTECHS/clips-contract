//! Transfer recipient guard (Issue #1025).
//!
//! Centralises every validation that concerns the **destination** of an NFT
//! transfer.  Callers only need to invoke one function — [`check_recipient`]
//! — and all recipient-side invariants are enforced in a defined order.
//!
//! # Acceptance criteria
//!
//! | Criterion | Guard function |
//! |-----------|---------------|
//! | Reject invalid recipient addresses | [`check_not_contract_address`] |
//! | Prevent transfers to prohibited recipients (blacklist) | [`check_not_blacklisted`] |
//! | Prevent self-transfers | [`check_not_self_transfer`] |
//! | Integrate with existing recipient validation | [`crate::transfer_guard`] delegates to [`check_recipient`] |
//!
//! # Guard order
//!
//! [`check_recipient`] runs checks in the following sequence, returning the
//! first error it encounters:
//!
//! 1. **Self-transfer** — `from` and `to` must be different addresses
//!    ([`Error::SelfTransferNotAllowed`]).
//! 2. **Contract self-address** — `to` must not be the contract's own address
//!    ([`Error::InvalidRecipient`]).
//! 3. **Blacklist** — `to` must not appear on the contract blacklist
//!    ([`Error::InvalidAddress`]).
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full recipient check — called by transfer_guard:
//! transfer_recipient_guard::check_recipient(&env, &from, &to)?;
//!
//! // Individual probes — for callers that need granular control:
//! transfer_recipient_guard::check_not_self_transfer(&from, &to)?;
//! transfer_recipient_guard::check_not_contract_address(&env, &to)?;
//! transfer_recipient_guard::check_not_blacklisted(&env, &to)?;
//! ```

use soroban_sdk::{Address, Env};

use crate::blacklist;
use crate::types::Error;

// ─── Primary integration point ────────────────────────────────────────────────

/// Validate every recipient-side invariant before an NFT transfer executes.
///
/// This is the function [`crate::transfer_guard`] calls to enforce all
/// recipient constraints in one shot.  Individual guard functions are also
/// public for callers that need finer-grained control.
///
/// # Arguments
/// * `env`  — Contract environment.
/// * `from` — Current owner / sender of the token.
/// * `to`   — Prospective new owner / destination of the token.
///
/// # Guard order
/// 1. `from` ≠ `to` — prevents self-transfers.
/// 2. `to` ≠ contract address — rejects the contract as a destination.
/// 3. `to` not blacklisted — blocks prohibited recipients.
///
/// # Errors
/// | Error | Triggered when |
/// |-------|---------------|
/// | [`Error::SelfTransferNotAllowed`] | `from` and `to` are the same address. |
/// | [`Error::InvalidRecipient`]        | `to` is the contract's own address. |
/// | [`Error::InvalidAddress`]          | `to` is on the blacklist. |
pub fn check_recipient(env: &Env, from: &Address, to: &Address) -> Result<(), Error> {
    check_not_self_transfer(from, to)?;
    check_not_contract_address(env, to)?;
    check_not_blacklisted(env, to)?;
    Ok(())
}

// ─── Individual guards ────────────────────────────────────────────────────────

/// Reject transfers where sender and recipient are the same address.
///
/// A transfer that does not change ownership is a no-op at best and a source
/// of subtle accounting bugs at worst.  Rejecting it early ensures every
/// successful transfer is a real ownership change.
///
/// # Errors
/// - [`Error::SelfTransferNotAllowed`] — `from == to`.
pub fn check_not_self_transfer(from: &Address, to: &Address) -> Result<(), Error> {
    if from == to {
        return Err(Error::SelfTransferNotAllowed);
    }
    Ok(())
}

/// Reject the contract's own address as a transfer destination.
///
/// The contract cannot hold NFT ownership in a way that allows subsequent
/// transfers, so sending to itself effectively destroys the token without a
/// proper burn event.  This guard covers the acceptance criterion
/// "reject invalid recipient addresses" (issue #724 / #1025).
///
/// # Errors
/// - [`Error::InvalidRecipient`] — `to` is [`Env::current_contract_address`].
pub fn check_not_contract_address(env: &Env, to: &Address) -> Result<(), Error> {
    if *to == env.current_contract_address() {
        return Err(Error::InvalidRecipient);
    }
    Ok(())
}

/// Reject a recipient that appears on the contract blacklist.
///
/// A blacklisted address has been administratively prohibited from
/// participating in any contract interaction, including receiving tokens.
/// This guard covers the acceptance criterion "prevent transfers to
/// prohibited recipients" (issue #728 / #1025).
///
/// # Errors
/// - [`Error::InvalidAddress`] — `to` is on the blacklist.
pub fn check_not_blacklisted(env: &Env, to: &Address) -> Result<(), Error> {
    if blacklist::is_blacklisted(env, to) {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blacklist;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Register the contract and run `f` inside its execution context so that
    /// `env.current_contract_address()` returns a stable, known address.
    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    // ── check_not_self_transfer ───────────────────────────────────────────────

    #[test]
    fn self_transfer_is_rejected() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(
                check_not_self_transfer(&owner, &owner),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn different_addresses_pass_self_transfer_check() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            assert!(check_not_self_transfer(&from, &to).is_ok());
        });
    }

    #[test]
    fn self_transfer_check_is_symmetric() {
        // The check is on identity, not ordering — swapping from/to with the
        // same address still fails.
        with_contract(|env| {
            let addr = Address::generate(env);
            assert_eq!(
                check_not_self_transfer(&addr, &addr),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    // ── check_not_contract_address ────────────────────────────────────────────

    #[test]
    fn contract_address_as_recipient_is_rejected() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            assert_eq!(
                check_not_contract_address(env, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn normal_wallet_passes_contract_address_check() {
        with_contract(|env| {
            let wallet = Address::generate(env);
            assert!(check_not_contract_address(env, &wallet).is_ok());
        });
    }

    #[test]
    fn contract_address_check_uses_current_contract() {
        // Different generated addresses are never the current contract.
        with_contract(|env| {
            for _ in 0..3 {
                let wallet = Address::generate(env);
                assert!(check_not_contract_address(env, &wallet).is_ok());
            }
        });
    }

    // ── check_not_blacklisted (recipient-only) ────────────────────────────────

    #[test]
    fn non_blacklisted_recipient_passes() {
        with_contract(|env| {
            let to = Address::generate(env);
            assert!(check_not_blacklisted(env, &to).is_ok());
        });
    }

    #[test]
    fn blacklisted_recipient_is_rejected() {
        with_contract(|env| {
            let to = Address::generate(env);
            blacklist::add_wallet(env, &to);
            assert_eq!(check_not_blacklisted(env, &to), Err(Error::InvalidAddress));
        });
    }

    #[test]
    fn recipient_allowed_after_removal_from_blacklist() {
        with_contract(|env| {
            let to = Address::generate(env);
            blacklist::add_wallet(env, &to);
            blacklist::remove_wallet(env, &to);
            assert!(check_not_blacklisted(env, &to).is_ok());
        });
    }

    #[test]
    fn blacklisting_sender_does_not_affect_recipient_guard() {
        // check_not_blacklisted only checks `to`; the sender blacklist check
        // is a separate concern handled by transfer_guard.
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            blacklist::add_wallet(env, &from); // sender is blacklisted
                                               // Recipient check should still pass because `to` is clean.
            assert!(check_not_blacklisted(env, &to).is_ok());
        });
    }

    // ── check_recipient (full pipeline) ──────────────────────────────────────

    #[test]
    fn valid_recipient_passes_all_checks() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            assert!(check_recipient(env, &from, &to).is_ok());
        });
    }

    #[test]
    fn full_check_rejects_self_transfer_first() {
        with_contract(|env| {
            let owner = Address::generate(env);
            assert_eq!(
                check_recipient(env, &owner, &owner),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn full_check_rejects_contract_as_recipient() {
        with_contract(|env| {
            let from = Address::generate(env);
            let contract = env.current_contract_address();
            assert_eq!(
                check_recipient(env, &from, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn full_check_rejects_blacklisted_recipient() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            blacklist::add_wallet(env, &to);
            assert_eq!(check_recipient(env, &from, &to), Err(Error::InvalidAddress));
        });
    }

    #[test]
    fn full_check_rejects_contract_even_when_different_from_sender() {
        // Ensure the contract-address check cannot be bypassed by ensuring
        // `from` is distinct from the contract address.
        with_contract(|env| {
            let from = Address::generate(env);
            let contract = env.current_contract_address();
            assert_ne!(from, contract);
            assert_eq!(
                check_recipient(env, &from, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    // ── Guard ordering ────────────────────────────────────────────────────────

    #[test]
    fn self_transfer_error_takes_priority_over_blacklist() {
        // When `from == to` and `to` is also blacklisted, SelfTransferNotAllowed
        // must be returned (self-transfer is checked first).
        with_contract(|env| {
            let addr = Address::generate(env);
            blacklist::add_wallet(env, &addr);
            assert_eq!(
                check_recipient(env, &addr, &addr),
                Err(Error::SelfTransferNotAllowed)
            );
        });
    }

    #[test]
    fn contract_address_error_takes_priority_over_blacklist() {
        // Contract address is checked before the blacklist; even if the contract
        // address were blacklisted, InvalidRecipient is returned first.
        with_contract(|env| {
            let from = Address::generate(env);
            let contract = env.current_contract_address();
            // Blacklist the contract address too.
            blacklist::add_wallet(env, &contract);
            assert_eq!(
                check_recipient(env, &from, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn blacklist_check_runs_when_self_and_contract_checks_pass() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            // Distinct wallets, neither is the contract — blacklist is the
            // last remaining gate.
            blacklist::add_wallet(env, &to);
            assert_eq!(check_recipient(env, &from, &to), Err(Error::InvalidAddress));
        });
    }
}
