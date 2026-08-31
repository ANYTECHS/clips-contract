use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{NFTFrozenEvent, TokenId};

/// Emit the event after an NFT is frozen.
pub fn emit_nft_frozen(
    env: &Env,
    token_id: TokenId,
    caller: &Address,
    reason: Option<&String>,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_frz"),),
        NFTFrozenEvent {
            token_id,
            caller: caller.clone(),
            reason: reason.cloned(),
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freeze_event_includes_token_caller_reason_and_timestamp() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let token_id = 42u32;
        let reason = String::from_str(&env, "investigating wallet compromise");

        emit_nft_frozen(&env, token_id, &caller, Some(&reason), 1_720_000_000);

        let event = env
            .events()
            .all()
            .events()
            .iter()
            .find_map(|(_, data): (soroban_sdk::Vec<soroban_sdk::Val>, NFTFrozenEvent)| Some(data))
            .expect("freeze event missing");

        assert_eq!(event.token_id, token_id);
        assert_eq!(event.caller, caller);
        assert_eq!(event.reason, Some(reason));
        assert_eq!(event.timestamp, 1_720_000_000);
    }
}
