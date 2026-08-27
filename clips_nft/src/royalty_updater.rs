//! Royalty update function (issue #793).
//!
//! [`update_royalty_configuration`] is the single authorized entry point for
//! modifying an existing token's royalty configuration — recipients, split, and
//! payment asset. Every change is validated before it is persisted:
//!
//! 1. **Operational guard** — the contract must not be paused
//!    ([`crate::pause_guard::require_not_paused`]);
//! 2. **State validation** — the configuration must be in an updatable state:
//!    the token must exist and must not be frozen ([#795]);
//! 3. **Authorization** — the caller must be the admin, creator, or owner
//!    ([#792]);
//! 4. **Value validation** — the new recipients, basis points, and asset must
//!    be individually valid;
//! 5. **Persistence** — the full config plus the O(1) recipient/percentage
//!    indexes are updated atomically.

use soroban_sdk::{Address, Env};

use crate::royalty_asset_validator::validate_royalty_asset;
use crate::royalty_authorization::authorize_royalty_update;
use crate::royalty_lifecycle::validate_state_for_update;
use crate::royalty_recipient::set_royalty_recipient;
use crate::royalty_recipient_struct::validate_royalty_recipient_struct;
use crate::royalty_validator::validate_royalty;
use crate::types::{Error, Royalty, TokenId};

/// Update the royalty configuration of `token_id`.
///
/// The caller must be the contract admin, the token creator, or the token
/// owner ([`Error::UnauthorizedConfigurationUpdate`] otherwise). The update
/// is only permitted while the configuration is in the **Active** lifecycle
/// state ([`Error::TokenNotFound`] for unknown tokens,
/// [`Error::RoyaltyFrozen`] for frozen ones).
///
/// On success the full [`Royalty`] configuration and its derived indexes
/// (recipient, percentage) are persisted.
///
/// # Errors
/// - [`Error::ContractPaused`]                — contract is paused.
/// - [`Error::TokenNotFound`]                 — token has no royalty config.
/// - [`Error::RoyaltyFrozen`]                 — configuration is frozen.
/// - [`Error::UnauthorizedConfigurationUpdate`] — caller not authorized.
/// - [`Error::InvalidBasisPoints`]            — new config is out of range.
/// - [`Error::InvalidRecipient`]              — new recipient is invalid.
/// - [`Error::UnsupportedAsset`]              — new asset is unsupported.
pub fn update_royalty_configuration(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
    new_royalty: &Royalty,
) -> Result<(), Error> {
    crate::pause_guard::require_not_paused(env)?;
    // Issue #795: reject unknown/frozen states before any change.
    validate_state_for_update(env, token_id)?;
    // Issue #792: only admin / creator / owner may reconfigure.
    authorize_royalty_update(env, caller, token_id)?;
    // Issue #793: validate the incoming configuration.
    validate_royalty(new_royalty)?;
    validate_royalty_asset(env, &new_royalty.asset_address)?;
    for recipient in new_royalty.recipients.iter() {
        validate_royalty_recipient_struct(env, &recipient)?;
    }
    // Persist the full configuration and derived indexes.
    crate::token_storage::set_royalty(env, token_id, new_royalty);
    if let Some(first) = new_royalty.recipients.first() {
        set_royalty_recipient(env, token_id, &first.recipient);
    }
    let total_bps = new_royalty
        .recipients
        .iter()
        .fold(0u32, |acc, r| acc.saturating_add(r.basis_points));
    crate::royalty_percentage::set_royalty_percentage(env, token_id, total_bps)?;
    Ok(())
}

/// Convenience helper for callers that hold the token owner's address.
///
/// Returns the token's current owner when the ownership record exists; this
/// is used by index maintenance on update.
#[allow(dead_code)]
pub fn current_owner(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    crate::token_owner_storage::get_owner(env, token_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::royalty_freeze::freeze_royalty;
    use crate::token_storage;
    use crate::types::{DataKey, RoyaltyRecipient};
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    const TOKEN: TokenId = 1;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn royal(env: &Env, recipient: &Address, bps: u32) -> Royalty {
        Royalty {
            recipients: soroban_sdk::vec![
                env,
                RoyaltyRecipient {
                    recipient: recipient.clone(),
                    basis_points: bps,
                }
            ],
            asset_address: None,
        }
    }

    fn seed(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);
        let creator = Address::generate(env);
        let owner = Address::generate(env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        crate::creator_storage::set_creator(env, TOKEN, &creator);
        crate::token_owner_storage::save_owner(env, TOKEN, &owner);
        token_storage::set_royalty(env, TOKEN, &royal(env, &Address::generate(env), 500));
        crate::royalty_recipient::set_royalty_recipient(env, TOKEN, &Address::generate(env));
        crate::royalty_percentage::set_royalty_percentage(env, TOKEN, 500).unwrap();
        (admin, creator, owner)
    }

    #[test]
    fn creator_can_update_recipient_and_percentage() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            let new_recipient = Address::generate(env);

            let updated = royal(env, &new_recipient, 1_000);
            assert!(update_royalty_configuration(env, &creator, TOKEN, &updated).is_ok());

            assert_eq!(token_storage::get_royalty(env, TOKEN), Ok(updated));
            assert_eq!(
                crate::royalty_recipient::get_royalty_recipient(env, TOKEN),
                Ok(new_recipient)
            );
            assert_eq!(
                crate::royalty_percentage::get_royalty_percentage(env, TOKEN),
                Ok(1_000)
            );
        });
    }

    #[test]
    fn admin_can_update() {
        with_contract(|env| {
            let (admin, _, _) = seed(env);
            let updated = royal(env, &Address::generate(env), 750);
            assert!(update_royalty_configuration(env, &admin, TOKEN, &updated).is_ok());
        });
    }

    #[test]
    fn owner_can_update() {
        with_contract(|env| {
            let (_, _, owner) = seed(env);
            let updated = royal(env, &Address::generate(env), 250);
            assert!(update_royalty_configuration(env, &owner, TOKEN, &updated).is_ok());
        });
    }

    #[test]
    fn unauthorized_caller_rejected() {
        with_contract(|env| {
            let (_, _, _) = seed(env);
            let interloper = Address::generate(env);
            assert_eq!(
                update_royalty_configuration(
                    env,
                    &interloper,
                    TOKEN,
                    &royal(env, &Address::generate(env), 500),
                ),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
            // Existing configuration is untouched.
            assert_eq!(
                crate::royalty_percentage::get_royalty_percentage(env, TOKEN),
                Ok(500)
            );
        });
    }

    #[test]
    fn missing_token_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);
            assert_eq!(
                update_royalty_configuration(
                    env,
                    &admin,
                    999,
                    &royal(env, &Address::generate(env), 500),
                ),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn frozen_token_rejected() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            assert!(freeze_royalty(env, &creator, TOKEN).is_ok());
            assert_eq!(
                update_royalty_configuration(
                    env,
                    &creator,
                    TOKEN,
                    &royal(env, &Address::generate(env), 500),
                ),
                Err(Error::RoyaltyFrozen)
            );
            // Existing configuration is untouched.
            assert_eq!(
                crate::royalty_percentage::get_royalty_percentage(env, TOKEN),
                Ok(500)
            );
        });
    }

    #[test]
    fn invalid_new_royalty_rejected_atomically() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            assert_eq!(
                update_royalty_configuration(
                    env,
                    &creator,
                    TOKEN,
                    &royal(env, &Address::generate(env), 20_000),
                ),
                Err(Error::InvalidBasisPoints)
            );
            // Existing configuration is untouched.
            assert_eq!(
                crate::royalty_percentage::get_royalty_percentage(env, TOKEN),
                Ok(500)
            );
        });
    }

    #[test]
    fn unsupported_asset_rejected() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            let mut updated = royal(env, &Address::generate(env), 500);
            updated.asset_address = Some(Address::generate(env));
            assert_eq!(
                update_royalty_configuration(env, &creator, TOKEN, &updated),
                Err(Error::UnsupportedAsset)
            );
        });
    }

    #[test]
    fn paused_contract_rejected() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            crate::pause_state::save_pause_state(env, true);
            assert_eq!(
                update_royalty_configuration(
                    env,
                    &creator,
                    TOKEN,
                    &royal(env, &Address::generate(env), 500),
                ),
                Err(Error::ContractPaused)
            );
        });
    }

    #[test]
    fn multi_recipient_update_persists() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            let first = Address::generate(env);
            let second = Address::generate(env);
            let updated = Royalty {
                recipients: soroban_sdk::vec![
                    env,
                    RoyaltyRecipient {
                        recipient: first.clone(),
                        basis_points: 400
                    },
                    RoyaltyRecipient {
                        recipient: second.clone(),
                        basis_points: 300
                    },
                ],
                asset_address: None,
            };
            assert!(update_royalty_configuration(env, &creator, TOKEN, &updated).is_ok());

            assert_eq!(token_storage::get_royalty(env, TOKEN), Ok(updated));
            assert_eq!(
                crate::royalty_recipient::get_royalty_recipient(env, TOKEN),
                Ok(first)
            );
            assert_eq!(
                crate::royalty_percentage::get_royalty_percentage(env, TOKEN),
                Ok(700)
            );
        });
    }

    #[test]
    fn zero_percentage_update_is_permitted() {
        with_contract(|env| {
            let (_, creator, _) = seed(env);
            let updated = royal(env, &Address::generate(env), 0);
            assert!(update_royalty_configuration(env, &creator, TOKEN, &updated).is_ok());
            assert_eq!(
                crate::royalty_percentage::get_royalty_percentage(env, TOKEN),
                Ok(0)
            );
        });
    }
}
