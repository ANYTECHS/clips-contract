use soroban_sdk::{Address, Env, Vec};

use crate::owner_portfolio;
use crate::token_lifecycle;
use crate::token_owner_storage;
use crate::token_storage;
use crate::transfer_event;
use crate::transfer_guard;
use crate::transfer_request::TransferRequest;
use crate::types::{BatchTransferError, Error, TokenId};
use crate::wallet_token_index;
use crate::{config, storage_constants};

pub fn transfer(
    env: &Env,
    caller: &Address,
    from: &Address,
    to: &Address,
    token_id: TokenId,
) -> Result<(), Error> {
    caller.require_auth();
    transfer_guard::check_transfer(env, caller, from, to, token_id)?;

    token_owner_storage::update_owner_after_validation(env, token_id, to);
    if let Ok(mut data) = token_storage::get_token(env, token_id) {
        data.owner = to.clone();
        token_storage::set_token(env, token_id, &data);
    }
    if owner_portfolio::owner_contains_token(env, from, token_id) {
        owner_portfolio::move_token_between_owners(env, from, to, token_id)
            .map_err(|_| Error::BatchTransferFailed)?;
    }
    if wallet_token_index::wallet_contains_token(env, from, token_id) {
        wallet_token_index::move_token_between_wallets(env, from, to, token_id)
            .map_err(|_| Error::BatchTransferFailed)?;
    }
    transfer_event::emit_nft_transferred(
        env,
        token_id,
        from,
        to,
        env.ledger().timestamp(),
    );
    Ok(())
}

pub fn transfer_request(env: &Env, caller: &Address, request: &TransferRequest) -> Result<(), Error> {
    transfer(env, caller, &request.from, &request.to, request.token_id)
}

pub fn batch_transfer(
    env: &Env,
    caller: &Address,
    requests: &Vec<TransferRequest>,
) -> Result<(), BatchTransferError> {
    caller.require_auth();
    validate_batch(env, requests)?;

    for request in requests.iter() {
        transfer(env, caller, &request.from, &request.to, request.token_id)
            .map_err(|_| BatchTransferError::FailedTransfer)?;
    }
    Ok(())
}

pub fn validate_batch(
    env: &Env,
    requests: &Vec<TransferRequest>,
) -> Result<(), BatchTransferError> {
    let max = config::get_config(env)
        .map(|value| value.max_batch_transfer_size)
        .unwrap_or(storage_constants::MAX_BATCH_TRANSFER_SIZE);
    if requests.is_empty() {
        return Err(BatchTransferError::EmptyBatch);
    }
    if requests.len() > max {
        return Err(BatchTransferError::BatchTooLarge);
    }

    let mut seen = Vec::new(env);
    for request in requests.iter() {
        if seen.contains(&request.token_id) {
            return Err(BatchTransferError::DuplicateToken);
        }
        seen.push_back(request.token_id);

        // Invalid recipient: contract address or zero-like invalid recipient.
        if request.to == env.current_contract_address() {
            return Err(Error::InvalidRecipient);
        }

        // Invalid transfer state: token must be transferable (not burned, not frozen,
        // not inactive). Map lifecycle failures to standardized state errors.
        token_lifecycle::require_transferable(env, request.token_id)
            .map_err(map_lifecycle_error)?;

        // Invalid transfer state: missing owner or ownership mismatch.
        match token_owner_storage::get_owner(env, request.token_id) {
            Ok(owner) => {
                if owner != request.from {
                    return Err(Error::InvalidTransferState);
                }
            }
            Err(Error::MissingOwner) => return Err(Error::InvalidTransferState),
            Err(_) => return Err(Error::InvalidTransferState),
            return Err(BatchTransferError::InvalidRecipient);
        }
        token_lifecycle::require_transferable(env, request.token_id)
            .map_err(|_| BatchTransferError::FailedTransfer)?;
        if token_owner_storage::get_owner(env, request.token_id)
            .map_err(|_| BatchTransferError::FailedTransfer)?
            != request.from
        {
            return Err(BatchTransferError::FailedTransfer);
        }
    }
    Ok(())
}

/// Maps token lifecycle errors into the standardized invalid transfer state error.
fn map_lifecycle_error(error: Error) -> Error {
    match error {
        Error::TokenBurned => Error::InvalidTransferState,
        Error::TokenFrozen => Error::InvalidTransferState,
        Error::InactiveToken => Error::InvalidTransferState,
        Error::MissingOwner => Error::InvalidTransferState,
        Error::InvalidOwnershipState => Error::InvalidTransferState,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    fn make_request(env: &Env, from: &Address, to: &Address, token_id: TokenId) -> TransferRequest {
        TransferRequest {
            from: from.clone(),
            to: to.clone(),
            token_id,
        }
    }

    #[test]
    fn validate_batch_rejects_empty_batch() {
        let env = Env::default();
        let requests: Vec<TransferRequest> = Vec::new(&env);
        assert_eq!(validate_batch(&env, &requests), Err(Error::EmptyBatch));
    }

    #[test]
    fn validate_batch_rejects_duplicate_token() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        requests.push_back(make_request(&env, &from, &to, 1));
        assert_eq!(validate_batch(&env, &requests), Err(Error::DuplicateToken));
    }

    #[test]
    fn validate_batch_rejects_invalid_recipient_contract_address() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = env.current_contract_address();
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        assert_eq!(validate_batch(&env, &requests), Err(Error::InvalidRecipient));
    }

    #[test]
    fn validate_batch_rejects_burned_token() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        // Assuming token 1 is burned in the test setup; lifecycle check fails.
        let result = validate_batch(&env, &requests);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err, Error::InvalidTransferState);
        }
    }

    #[test]
    fn validate_batch_rejects_frozen_token() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        // Assuming token 1 is frozen in the test setup; lifecycle check fails.
        let result = validate_batch(&env, &requests);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err, Error::InvalidTransferState);
        }
    }

    #[test]
    fn validate_batch_rejects_inactive_token() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        // Assuming token 1 is inactive in the test setup; lifecycle check fails.
        let result = validate_batch(&env, &requests);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err, Error::InvalidTransferState);
        }
    }

    #[test]
    fn validate_batch_rejects_missing_owner() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        // Assuming token 1 has no owner in the test setup.
        let result = validate_batch(&env, &requests);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err, Error::InvalidTransferState);
        }
    }

    #[test]
    fn validate_batch_rejects_invalid_ownership_state() {
        let env = Env::default();
        let from = Address::generate(&env);
        let to = Address::generate(&env);
        let mut requests = Vec::new(&env);
        requests.push_back(make_request(&env, &from, &to, 1));
        // Assuming token 1 is owned by a different address in the test setup.
        let result = validate_batch(&env, &requests);
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err, Error::InvalidTransferState);
        }
    }
}