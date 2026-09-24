//! NFT frozen event — emitted when a token's transfer ability is disabled.
//!
//! Resolves issue #917: emit an event whenever a ClipCash NFT is frozen,
//! including the token ID, caller, optional reason, and timestamp.
//!
//! # Event topic
//! - `"nft_froz"` — NFT frozen (transfer-disabled)
//!
//! This event is emitted after a successful [`crate::frozen_token::freeze_token`]
//! call, providing an immutable record of when and why a token was frozen.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::event_topics::TOPIC_FREEZE;
use crate::types::{NFTFrozenEvent, TokenId};

/// Emit the `"nft_froz"` event after an NFT is frozen.
///
/// Must be called **after** the token is frozen, so receiving the event
/// guarantees the token is already transfer-disabled on-chain.
///
/// # Arguments
/// * `env`      — Contract execution environment.
/// * `token_id` — On-chain token ID that was frozen.
/// * `caller`   — Address that initiated the freeze (usually the admin).
/// * `reason`   — Optional free-text reason for the freeze.
/// * `timestamp`— Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_frozen(
    env: &Env,
    token_id: TokenId,
    caller: &Address,
    reason: Option<&String>,
    timestamp: u64,
) {
    env.events().publish(
        (TOPIC_FREEZE,),
        NFTFrozenEvent {
            token_id,
            caller: caller.clone(),
            reason: reason.cloned(),
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_nft_frozen_event(
    env: &Env,
    token_id: TokenId,
    caller: &Address,
    reason: Option<&String>,
    timestamp: u64,
) -> NFTFrozenEvent {
    let _ = env; // env kept for API symmetry with emit_nft_frozen
    NFTFrozenEvent {
        token_id,
        caller: caller.clone(),
        reason: reason.cloned(),
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Events;

    #[test]
    fn freeze_event_includes_token_caller_reason_and_timestamp() {
        let env = Env::default();
        let caller = soroban_sdk::Address::generate(&env);
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

    #[test]
    fn freeze_event_without_reason() {
        let env = Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let token_id = 43u32;

        emit_nft_frozen(&env, token_id, &caller, None, 1_720_000_001);

        let event = env
            .events()
            .all()
            .events()
            .iter()
            .find_map(|(_, data): (soroban_sdk::Vec<soroban_sdk::Val>, NFTFrozenEvent)| Some(data))
            .expect("freeze event missing");

        assert_eq!(event.token_id, token_id);
        assert_eq!(event.caller, caller);
        assert_eq!(event.reason, None);
        assert_eq!(event.timestamp, 1_720_000_001);
    }

    #[test]
    fn build_nft_frozen_event_creates_valid_payload() {
        let env = Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let token_id = 44u32;
        let reason = String::from_str(&env, "policy violation");

        let payload = build_nft_frozen_event(&env, token_id, &caller, Some(&reason), 1_730_000_000);

        assert_eq!(payload.token_id, token_id);
        assert_eq!(payload.caller, caller);
        assert_eq!(payload.reason, Some(reason));
        assert_eq!(payload.timestamp, 1_730_000_000);
    }

    #[test]
    fn build_nft_frozen_event_without_reason() {
        let env = Env::default();
        let caller = soroban_sdk::Address::generate(&env);
        let token_id = 45u32;

        let payload = build_nft_frozen_event(&env, token_id, &caller, None, 1_740_000_000);

        assert_eq!(payload.token_id, token_id);
        assert_eq!(payload.caller, caller);
        assert_eq!(payload.reason, None);
        assert_eq!(payload.timestamp, 1_740_000_000);
    }
}
