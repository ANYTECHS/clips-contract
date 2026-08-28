//! Royalty validation pipeline (issue #796).
//!
//! A centralized validation pipeline for **every** royalty operation. All
//! royalty entry points funnel through [`validate_royalty_operation`], which
//! validates, in order:
//!
//! 1. Contract pause state     — [`crate::pause_guard::require_not_paused`]
//! 2. Token existence          — royalty config is present for the token
//! 3. Caller authorization     — admin, token creator, or token owner
//! 4. Royalty state            — token is not royalty-frozen
//! 5. Royalty configuration    — recipients, basis points, and asset
//!
//! Each stage is individually exposed so callers can reuse a single step
//! without running the entire pipeline.

use soroban_sdk::{Address, Env};

use crate::pause_guard::require_not_paused;
use crate::royalty_asset_validator::validate_royalty_asset;
use crate::royalty_recipient_validator::validate_royalty_recipient;
use crate::royalty_validator::validate_royalty;
use crate::token_storage;
use crate::types::{DataKey, Error, Royalty, TokenId};

/// Stage 1 — reject the operation when the contract is paused.
pub fn validate_contract_not_paused(env: &Env) -> Result<(), Error> {
    require_not_paused(env)
}

/// Return `true` when the token's royalty configuration is frozen.
///
/// Frozen royalty configurations reject every subsequent update. The marker
/// is a plain `bool` under `DataKey::RoyaltyFrozen(token_id)` and defaults to
/// `false` when never set.
pub fn is_royalty_frozen(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .get::<_, bool>(&DataKey::RoyaltyFrozen(token_id))
        .unwrap_or(false)
}

/// Stage 2 — verify the token exists and has a royalty configuration.
pub fn validate_token_exists(env: &Env, token_id: TokenId) -> Result<(), Error> {
    token_storage::get_royalty(env, token_id)?;
    Ok(())
}

/// Stage 4 — verify the royalty lifecycle state permits an update.
///
/// Rejects non-existent tokens (via [`validate_token_exists`]) and tokens
/// whose royalty configuration has been frozen ([`Error::RoyaltyFrozen`]).
pub fn validate_royalty_state(env: &Env, token_id: TokenId) -> Result<(), Error> {
    validate_token_exists(env, token_id)?;
    if is_royalty_frozen(env, token_id) {
        return Err(Error::RoyaltyFrozen);
    }
    Ok(())
}

/// Stage 3 — verify the caller may modify the token's royalty configuration.
///
/// The following identities are authorized:
/// - the contract **admin** (stored at `DataKey::Admin`),
/// - the token **creator** ([`crate::creator_storage`]),
/// - the token **owner** ([`crate::token_owner_storage`]).
///
/// Caller authentication is enforced with `require_auth` for the matched
/// identity before the check succeeds.
///
/// # Errors
/// - [`Error::UnauthorizedConfigurationUpdate`] — no identity matches.
/// - [`Error::TokenNotFound`] — no creator/owner record exists to check.
pub fn authorize_royalty_update(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
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

    if let Ok(creator) = crate::creator_storage::get_creator(env, token_id) {
        if *caller == creator {
            caller.require_auth();
            return Ok(());
        }
    }

    if let Ok(owner) = crate::token_owner_storage::get_owner(env, token_id) {
        if *caller == owner {
            caller.require_auth();
            return Ok(());
        }
    }

    Err(Error::UnauthorizedConfigurationUpdate)
}

/// Stage 5 — validate the royalty configuration itself.
///
/// Checks:
/// - the configuration is non-empty and total `basis_points <= 10 000`,
/// - every recipient is a valid royalty wallet (not the contract itself),
/// - the payment asset (if any) is supported by the contract.
pub fn validate_royalty_configuration(env: &Env, royalty: &Royalty) -> Result<(), Error> {
    validate_royalty(royalty)?;
    validate_royalty_asset(env, &royalty.asset_address)?;
    let mut i: u32 = 0;
    while let Some(r) = royalty.recipients.get(i) {
        validate_royalty_recipient(env, &r.recipient)?;
        i += 1;
    }
    Ok(())
}

/// Run the complete royalty validation pipeline.
///
/// # Arguments
/// * `caller`   — address invoking the royalty operation.
/// * `token_id` — on-chain token identifier.
/// * `royalty`  — proposed royalty configuration (validated for structure).
///
/// # Errors
/// - [`Error::ContractPaused`]               — contract paused.
/// - [`Error::TokenNotFound`]                — token has no royalty config.
/// - [`Error::UnauthorizedConfigurationUpdate`] — caller not authorized.
/// - [`Error::RoyaltyFrozen`]                — royalty config is frozen.
/// - [`Error::InvalidBasisPoints`] / [`Error::InvalidRecipient`] /
///   [`Error::UnsupportedAsset`]             — configuration invalid.
pub fn validate_royalty_operation(
    env: &Env,
    caller: &Address,
    token_id: TokenId,
    royalty: &Royalty,
) -> Result<(), Error> {
    validate_contract_not_paused(env)?;
    validate_token_exists(env, token_id)?;
    authorize_royalty_update(env, caller, token_id)?;
    validate_royalty_state(env, token_id)?;
    validate_royalty_configuration(env, royalty)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyRecipient;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn royalty(env: &Env, bps: u32) -> Royalty {
        Royalty {
            recipients: soroban_sdk::vec![env, RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: bps,
            }],
            asset_address: None,
        }
    }

    fn register_token(env: &Env, token_id: TokenId, admin: &Address, owner: &Address) {
        env.storage().instance().set(&DataKey::Admin, admin);
        token_storage::set_royalty(env, token_id, &royalty(env, 500));
        crate::creator_storage::set_creator(env, token_id, owner);
        crate::token_owner_storage::save_owner(env, token_id, owner);
    }

    #[test]
    fn valid_assignment_passes_pipeline() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let caller = Address::generate(env);
            register_token(env, 1, &admin, &caller);
            let new_royalty = royalty(env, 1_000);
            assert!(validate_royalty_operation(env, &caller, 1, &new_royalty).is_ok());
        });
    }

    #[test]
    fn invalid_recipient_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let caller = Address::generate(env);
            register_token(env, 2, &admin, &caller);

            let contract = env.current_contract_address();
            let bad = Royalty {
                recipients: soroban_sdk::vec![env, RoyaltyRecipient {
                    recipient: contract,
                    basis_points: 500,
                }],
                asset_address: None,
            };
            assert_eq!(
                validate_royalty_operation(env, &caller, 2, &bad),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn invalid_royalty_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let caller = Address::generate(env);
            register_token(env, 3, &admin, &caller);

            let empty = Royalty {
                recipients: soroban_sdk::Vec::new(env),
                asset_address: None,
            };
            assert_eq!(
                validate_royalty_operation(env, &caller, 3, &empty),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn nonexistent_nft_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let caller = Address::generate(env);
            env.storage().instance().set(&DataKey::Admin, &admin);
            assert_eq!(
                validate_royalty_operation(env, &caller, 999, &royalty(env, 500)),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn unauthorized_update_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            let interloper = Address::generate(env);
            register_token(env, 4, &admin, &owner);
            assert_eq!(
                validate_royalty_operation(env, &interloper, 4, &royalty(env, 500)),
                Err(Error::UnauthorizedConfigurationUpdate)
            );
        });
    }

    #[test]
    fn frozen_royalty_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            register_token(env, 5, &admin, &owner);
            env.storage()
                .persistent()
                .set(&DataKey::RoyaltyFrozen(5), &true);
            let frozen = is_royalty_frozen(env, 5);
            assert!(frozen);
            assert_eq!(
                validate_royalty_state(env, 5),
                Err(Error::RoyaltyFrozen)
            );
        });
    }

    #[test]
    fn max_royalty_accepted_and_above_rejected() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            register_token(env, 6, &admin, &owner);
            assert!(validate_royalty_configuration(env, &royalty(env, 10_000)).is_ok());
            assert_eq!(
                validate_royalty_configuration(env, &royalty(env, 10_001)),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn paused_contract_rejects_all() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let owner = Address::generate(env);
            register_token(env, 7, &admin, &owner);
            crate::pause_state::save_pause_state(env, true);
            assert_eq!(
                validate_royalty_operation(env, &owner, 7, &royalty(env, 500)),
                Err(Error::ContractPaused)
            );
        });
    }
}