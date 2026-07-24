//! Creator storage — records the creator wallet for every NFT.
//!
//! # Storage
//! Key: `DataKey::Creator(token_id)` (persistent storage)

use soroban_sdk::{Address, Env};

use crate::creator_event;
use crate::types::{DataKey, Error, TokenId};

/// Save the creator wallet for a token (no event).
pub fn set_creator(env: &Env, token_id: TokenId, creator: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Creator(token_id), creator);
}

/// Assign a creator to a newly minted NFT and emit [`CreatorAssignedEvent`].
///
/// # Arguments
/// * `token_id` — Newly minted token.
/// * `creator`  — Creator wallet.
/// * `clip_id`  — Linked off-chain clip identifier.
pub fn assign_creator(
    env: &Env,
    token_id: TokenId,
    creator: &Address,
    clip_id: u32,
) -> Result<(), Error> {
    set_creator(env, token_id, creator);
    let timestamp = env.ledger().timestamp();
    creator_event::emit_creator_assigned(env, token_id, creator, clip_id, timestamp);
    Ok(())
}

/// Read the creator wallet for a token.
///
/// Returns `Err(TokenNotFound)` if no creator has been recorded.
pub fn get_creator(env: &Env, token_id: TokenId) -> Result<Address, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Creator(token_id))
        .ok_or(Error::TokenNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger},
        Address, Env,
    };

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn assign_creator_persists_and_emits() {
        with_contract(|env| {
            env.ledger().set_timestamp(1_700_000_123);
            let creator = Address::generate(env);
            assign_creator(env, 5, &creator, 99).unwrap();
            assert_eq!(get_creator(env, 5).unwrap(), creator);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }
}
