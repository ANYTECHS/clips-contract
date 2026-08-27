use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{RoyaltyFrozenEvent, TokenId};

/// Emit the event after royalty configuration has been permanently frozen.
pub fn emit_royalty_frozen(env: &Env, token_id: TokenId, caller: &Address, timestamp: u64) {
    env.events().publish(
        (symbol_short!("ryl_frz"),),
        RoyaltyFrozenEvent {
            token_id,
            caller: caller.clone(),
            timestamp,
        },
    );
}