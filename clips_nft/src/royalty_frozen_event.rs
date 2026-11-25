use soroban_sdk::{Address, Env};
use crate::event_topics::TOPIC_ROYALTY_FROZEN;

use crate::types::{RoyaltyFrozenEvent, TokenId};

/// Emit the event after royalty configuration has been permanently frozen.
pub fn emit_royalty_frozen(env: &Env, token_id: TokenId, caller: &Address, timestamp: u64) {
    env.events().publish(
        (TOPIC_ROYALTY_FROZEN,),
        RoyaltyFrozenEvent {
            token_id,
            caller: caller.clone(),
            timestamp,
        },
    );
}
