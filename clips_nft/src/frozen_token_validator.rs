//! Operation policy for frozen NFTs.
//!
//! Frozen tokens cannot be moved, listed, or modified. Read-only inspection
//! and unfreezing remain permitted so that a frozen token can be reviewed and
//! restored. Token existence and frozen status are resolved through
//! [`crate::token_state_validator`].

use soroban_sdk::Env;

use crate::frozen_token;
use crate::token_owner_storage;
use crate::types::{Error, TokenId};

/// Operations whose availability is affected by a token's frozen status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrozenTokenOperation {
    /// Move ownership to another address.
    Transfer,
    /// Permanently destroy the token.
    Burn,
    /// Change token metadata.
    UpdateMetadata,
    /// Create a marketplace listing for the token.
    List,
    /// Complete a purchase that changes token ownership.
    Purchase,
    /// Grant or revoke permission to transfer the token.
    ManageApproval,
    /// Change royalty settings associated with the token.
    UpdateRoyalty,
    /// Read token state without changing it.
    Read,
    /// Remove the frozen marker to restore token functionality.
    Unfreeze,
}

/// Return whether `token_id` is frozen, rejecting unknown token IDs.
pub fn is_token_frozen(env: &Env, token_id: TokenId) -> Result<bool, Error> {
    token_owner_storage::get_owner(env, token_id)?;
    Ok(frozen_token::is_frozen(env, token_id))
}

/// Validate whether `operation` is permitted for `token_id`.
///
/// Active tokens permit all operations. Frozen tokens permit only read-only
/// inspection and unfreezing.
///
/// # Errors
/// - [`Error::TokenNotFound`] if the token does not exist.
/// - [`Error::Unauthorized`] if a prohibited operation is attempted against
///   a frozen token.
pub fn validate_token_operation(
    env: &Env,
    token_id: TokenId,
    operation: FrozenTokenOperation,
) -> Result<(), Error> {
    if !is_token_frozen(env, token_id)? {
        return Ok(());
    }

    match operation {
        FrozenTokenOperation::Read | FrozenTokenOperation::Unfreeze => Ok(()),
        FrozenTokenOperation::Transfer
        | FrozenTokenOperation::Burn
        | FrozenTokenOperation::UpdateMetadata
        | FrozenTokenOperation::List
        | FrozenTokenOperation::Purchase
        | FrozenTokenOperation::ManageApproval
        | FrozenTokenOperation::UpdateRoyalty => Err(Error::Unauthorized),
    }
}

/// Validate a transfer specifically; call before any ownership writes.
pub fn validate_transfer(env: &Env, token_id: TokenId) -> Result<(), Error> {
    validate_token_operation(env, token_id, FrozenTokenOperation::Transfer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frozen_token;
    use crate::token_owner_storage::save_owner;
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

    fn create_token(env: &Env, token_id: TokenId, frozen: bool) {
        let owner = Address::generate(env);
        save_owner(env, token_id, &owner);
        if frozen {
            frozen_token::freeze_token(env, token_id);
        }
    }

    #[test]
    fn detects_active_and_frozen_tokens() {
        with_contract(|env| {
            create_token(env, 1, false);
            create_token(env, 2, true);

            assert_eq!(is_token_frozen(env, 1), Ok(false));
            assert_eq!(is_token_frozen(env, 2), Ok(true));
        });
    }

    #[test]
    fn rejects_operations_prohibited_while_frozen() {
        with_contract(|env| {
            let token_id = 3;
            create_token(env, token_id, true);

            for operation in [
                FrozenTokenOperation::Transfer,
                FrozenTokenOperation::Burn,
                FrozenTokenOperation::UpdateMetadata,
                FrozenTokenOperation::List,
                FrozenTokenOperation::Purchase,
                FrozenTokenOperation::ManageApproval,
                FrozenTokenOperation::UpdateRoyalty,
            ] {
                assert_eq!(
                    validate_token_operation(env, token_id, operation),
                    Err(Error::Unauthorized)
                );
            }
        });
    }

    #[test]
    fn permits_read_and_unfreeze_while_frozen() {
        with_contract(|env| {
            let token_id = 4;
            create_token(env, token_id, true);

            assert_eq!(
                validate_token_operation(env, token_id, FrozenTokenOperation::Read),
                Ok(())
            );
            assert_eq!(
                validate_token_operation(env, token_id, FrozenTokenOperation::Unfreeze),
                Ok(())
            );
        });
    }

    #[test]
    fn permits_all_operations_for_active_tokens() {
        with_contract(|env| {
            let token_id = 5;
            create_token(env, token_id, false);

            for operation in [
                FrozenTokenOperation::Transfer,
                FrozenTokenOperation::Burn,
                FrozenTokenOperation::UpdateMetadata,
                FrozenTokenOperation::List,
                FrozenTokenOperation::Purchase,
                FrozenTokenOperation::ManageApproval,
                FrozenTokenOperation::UpdateRoyalty,
                FrozenTokenOperation::Read,
                FrozenTokenOperation::Unfreeze,
            ] {
                assert_eq!(validate_token_operation(env, token_id, operation), Ok(()));
            }
        });
    }

    #[test]
    fn rejects_unknown_tokens() {
        with_contract(|env| {
            assert_eq!(is_token_frozen(env, 999), Err(Error::TokenNotFound));
            assert_eq!(
                validate_token_operation(env, 999, FrozenTokenOperation::Read),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn transfer_helper_rejects_frozen_tokens() {
        with_contract(|env| {
            let token_id = 6;
            create_token(env, token_id, true);
            assert_eq!(validate_transfer(env, token_id), Err(Error::Unauthorized));
        });
    }
}
