use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{NFTUnfrozenEvent, TokenId};

/// Emit the event after a token's frozen state has been removed.
pub fn emit_nft_unfrozen(env: &Env, token_id: TokenId, caller: &Address, timestamp: u64) {
    env.events().publish(
        (symbol_short!("nft_unfrz"),),
        NFTUnfrozenEvent {
            token_id,
            caller: caller.clone(),
            timestamp,
        },
    );
}
