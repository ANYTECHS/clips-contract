use soroban_sdk::{symbol_short, Address, Env};

use crate::token_owner_storage;
use crate::token_approval;
use crate::transfer_guard;
use crate::transfer_request::{BatchTransferRequest, TransferRequest};
use crate::types::{Error, TransferEvent};
use crate::wallet_token_index;

/// Execute a single NFT transfer.
///
/// This delegates to `transfer_guard::check_transfer` to validate ownership,
/// operator approvals, token existence, frozen status, and blacklist status.
/// Once validated, it:
/// 1. Clears any token approvals.
/// 2. Updates the wallet indexes.
/// 3. Changes ownership.
/// 4. Emits a `TransferEvent`.
pub fn execute_transfer(
    env: &Env,
    caller: &Address,
    request: &TransferRequest,
) -> Result<(), Error> {
    // 1. Pre-transfer validations (ownership, token existence, frozen/active, approvals).
    transfer_guard::check_transfer(
        env,
        caller,
        &request.from,
        &request.to,
        request.token_id,
    )?;

    // 2. Clear token approvals before transferring.
    token_approval::remove_approval(env, request.token_id);

    // 3. Update wallet indexes.
    wallet_token_index::remove_token_from_wallet(env, &request.from, request.token_id);
    if wallet_token_index::add_token_to_wallet(env, &request.to, request.token_id).is_err() {
        return Err(Error::DuplicateWalletEntry);
    }

    // 4. Change token owner.
    token_owner_storage::update_owner_after_validation(env, request.token_id, &request.to);

    // 5. Emit transfer event.
    env.events().publish(
        (symbol_short!("transfer"),),
        TransferEvent {
            from: request.from.clone(),
            to: request.to.clone(),
            token_id: request.token_id,
        },
    );

    Ok(())
}

/// Execute a batch of NFT transfers.
///
/// Runs `execute_transfer` for each request in the batch sequentially. If any
/// single transfer fails, the entire batch is rolled back by Soroban's
/// atomic execution model.
pub fn execute_batch_transfer(
    env: &Env,
    caller: &Address,
    batch: &BatchTransferRequest,
) -> Result<(), Error> {
    for req in batch.requests.iter() {
        execute_transfer(env, caller, &req)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use crate::types::TokenId;
    use soroban_sdk::{testutils::{Address as _, Events}, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn setup_token(env: &Env, token_id: TokenId, owner: &Address) {
        token_owner_storage::assign_owner(env, token_id, owner, token_id).unwrap();
        wallet_token_index::add_token_to_wallet(env, owner, token_id).unwrap();
    }

    #[test]
    fn transfer_updates_ownership_and_indexes() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            setup_token(env, 1, &from);

            let req = TransferRequest {
                from: from.clone(),
                to: to.clone(),
                token_id: 1,
                timestamp: None,
                memo: None,
            };

            execute_transfer(env, &from, &req).unwrap();

            // Ownership updated
            assert_eq!(token_owner_storage::get_owner(env, 1).unwrap(), to);

            // Indexes updated
            let from_tokens = wallet_token_index::get_wallet_tokens(env, &from);
            assert_eq!(from_tokens.len(), 0);

            let to_tokens = wallet_token_index::get_wallet_tokens(env, &to);
            assert_eq!(to_tokens.len(), 1);
            assert_eq!(to_tokens.get(0).unwrap(), 1);

            // Event emitted
            let events = env.events().all();
            assert!(events.len() > 0);
        });
    }

    #[test]
    fn transfer_clears_approvals() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let operator = Address::generate(env);
            setup_token(env, 2, &from);

            token_approval::save_approval(env, 2, &operator);
            assert_eq!(token_approval::get_approval(env, 2).unwrap(), operator);

            let req = TransferRequest {
                from: from.clone(),
                to: to.clone(),
                token_id: 2,
                timestamp: None,
                memo: None,
            };

            execute_transfer(env, &from, &req).unwrap();

            assert!(token_approval::get_approval(env, 2).is_none());
        });
    }

    #[test]
    fn batch_transfer_processes_multiple_requests() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to1 = Address::generate(env);
            let to2 = Address::generate(env);
            setup_token(env, 10, &from);
            setup_token(env, 11, &from);

            let req1 = TransferRequest {
                from: from.clone(),
                to: to1.clone(),
                token_id: 10,
                timestamp: None,
                memo: None,
            };
            let req2 = TransferRequest {
                from: from.clone(),
                to: to2.clone(),
                token_id: 11,
                timestamp: None,
                memo: None,
            };
            
            let mut requests = soroban_sdk::Vec::new(env);
            requests.push_back(req1);
            requests.push_back(req2);
            let batch = BatchTransferRequest { requests };

            execute_batch_transfer(env, &from, &batch).unwrap();

            assert_eq!(token_owner_storage::get_owner(env, 10).unwrap(), to1);
            assert_eq!(token_owner_storage::get_owner(env, 11).unwrap(), to2);
            assert_eq!(wallet_token_index::get_wallet_tokens(env, &from).len(), 0);
        });
    }
}
