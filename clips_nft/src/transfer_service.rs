use soroban_sdk::{Address, Env, Vec};

use crate::owner_portfolio;
use crate::token_lifecycle;
use crate::token_owner_storage;
use crate::token_storage;
use crate::transfer_event;
use crate::transfer_guard;
use crate::transfer_request::TransferRequest;
use crate::types::{Error, TokenId};
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
) -> Result<(), Error> {
    caller.require_auth();
    validate_batch(env, requests)?;

    for request in requests.iter() {
        transfer(env, caller, &request.from, &request.to, request.token_id)?;
    }
    Ok(())
}

pub fn validate_batch(env: &Env, requests: &Vec<TransferRequest>) -> Result<(), Error> {
    let max = config::get_config(env)
        .map(|value| value.max_batch_transfer_size)
        .unwrap_or(storage_constants::MAX_BATCH_TRANSFER_SIZE);
    if requests.is_empty() {
        return Err(Error::EmptyBatch);
    }
    if requests.len() > max {
        return Err(Error::BatchTooLarge);
    }

    let mut seen = Vec::new(env);
    for request in requests.iter() {
        if seen.contains(&request.token_id) {
            return Err(Error::DuplicateToken);
        }
        seen.push_back(request.token_id);
        if request.to == env.current_contract_address() {
            return Err(Error::InvalidBatchRecipient);
        }
        token_lifecycle::require_transferable(env, request.token_id)?;
        if token_owner_storage::get_owner(env, request.token_id)? != request.from {
            return Err(Error::InvalidOwnershipState);
        }
    }
    Ok(())
}
