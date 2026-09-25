use soroban_sdk::{Address, Env};

use crate::burn_event;
use crate::frozen_token;
use crate::operator_approval;
use crate::owner_portfolio;
use crate::token_approval;
use crate::token_owner_storage;
use crate::token_storage;
use crate::types::{DataKey, Error, TokenId};
use crate::wallet_token_index;

pub fn is_burned(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::BurnedToken(token_id))
}

pub fn require_active(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if is_burned(env, token_id) {
        return Err(Error::AlreadyBurned);
    }
    if !token_storage::token_exists(env, token_id) {
        if token_owner_storage::has_owner(env, token_id) {
            return Err(Error::InvalidTransferState);
        }
        return Err(Error::TokenNotFound);
    }
    if !token_owner_storage::has_owner(env, token_id) {
        return Err(Error::MissingOwner);
    }
    Ok(())
}

pub fn require_transferable(env: &Env, token_id: TokenId) -> Result<(), Error> {
    require_active(env, token_id)?;
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::InvalidTransferState);
    }
    Ok(())
}

pub fn freeze_token(env: &Env, token_id: TokenId) -> Result<(), Error> {
    require_active(env, token_id)?;
    if frozen_token::is_frozen(env, token_id) {
        return Err(Error::AlreadyFrozen);
    }
    frozen_token::freeze_token(env, token_id);
    Ok(())
}

pub fn unfreeze_token(env: &Env, token_id: TokenId) -> Result<(), Error> {
    if is_burned(env, token_id) {
        return Err(Error::AlreadyBurned);
    }
    if !frozen_token::is_frozen(env, token_id) {
        return Err(Error::AlreadyUnfrozen);
    }
    frozen_token::unfreeze_token(env, token_id);
    Ok(())
}

pub fn burn_token(env: &Env, caller: &Address, token_id: TokenId) -> Result<Address, Error> {
    require_active(env, token_id)?;
    let owner = token_owner_storage::get_owner(env, token_id)?;
    if caller != &owner && !operator_approval::is_operator(env, &owner, caller) {
        return Err(Error::UnauthorizedTransfer);
    }
    token_approval::remove_approval(env, token_id);
    owner_portfolio::remove_token_from_owner(env, &owner, token_id);
    wallet_token_index::remove_token_from_wallet(env, &owner, token_id);
    token_owner_storage::remove_owner(env, token_id);
    token_storage::remove_token(env, token_id);
    env.storage()
        .persistent()
        .set(&DataKey::BurnedToken(token_id), &true);
    burn_event::emit_nft_burned(
        env,
        token_id,
        &owner,
        caller,
        env.ledger().timestamp(),
    );
    Ok(owner)
}
