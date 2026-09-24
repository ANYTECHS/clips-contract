//! NFT unfrozen event — emitted when a token's frozen state is removed.
//!
//! Resolves issue #918: emit an event whenever a ClipCash NFT is unfrozen,
//! including the token ID, caller, and timestamp.
//!
//! # Event topic
//! - `"nft_unfrz"` — NFT unfrozen (transfer re-enabled)
//!
//! This event is emitted after a successful [`crate::frozen_token::unfreeze_token`]
//! call, providing an immutable record of when a frozen token was unfrozen.

use soroban_sdk::{symbol_short, Address, Env};

use crate::event_topics::TOPIC_UNFREEZE;
use crate::types::{NFTUnfrozenEvent, TokenId};

/// Emit the `"nft_unfrz"` event after an NFT has been unfrozen.
///
/// Must be called **after** the token is unfrozen, so receiving the event
/// guarantees the token is already transfer-enabled on-chain.
///
/// # Arguments
/// * `env`      — Contract execution environment.
/// * `token_id` — On-chain token ID that was unfrozen.
/// * `caller`   — Address that initiated the unfreeze (usually the admin).
/// * `timestamp`— Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_unfrozen(env: &Env, token_id: TokenId, caller: &Address, timestamp: u64) {
    env.events().publish(
        (TOPIC_UNFREEZE,),
        NFTUnfrozenEvent {
            token_id,
            caller: caller.clone(),
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_nft_unfrozen_event(
    env: &Env,
    token_id: TokenId,
    caller: &Address,
    timestamp: u64,
) -> NFTUnfrozenEvent {
    let _ = env; // env kept for API symmetry with emit_nft_unfrozen
    NFTUnfrozenEvent {
        token_id,
        caller: caller.clone(),
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};

    #[test]
    fn emits_event_with_correct_payload() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let token_id = 42u32;
        let timestamp = 1_720_000_000u64;

        emit_nft_unfrozen(&env, token_id, &caller, timestamp);

        let events = env.events().all().events();
        assert_eq!(events.len(), 1);

        let (_, data): (soroban_sdk::Vec<soroban_sdk::Val>, NFTUnfrozenEvent) = events[0]
            .clone()
            .into_val(&env)
            .try_into()
            .expect("event should deserialize to NFTUnfrozenEvent");

        assert_eq!(data.token_id, token_id);
        assert_eq!(data.caller, caller);
        assert_eq!(data.timestamp, timestamp);
    }

    #[test]
    fn build_creates_valid_event_payload() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let token_id = 99u32;
        let timestamp = 2_000_000_000u64;

        let payload = build_nft_unfrozen_event(&env, token_id, &caller, timestamp);

        assert_eq!(payload.token_id, token_id);
        assert_eq!(payload.caller, caller);
        assert_eq!(payload.timestamp, timestamp);
    }

    #[test]
    fn multiple_unfreezes_emit_separate_events() {
        let env = Env::default();
        let caller = Address::generate(&env);

        emit_nft_unfrozen(&env, 1, &caller, 100);
        emit_nft_unfrozen(&env, 2, &caller, 200);
        emit_nft_unfrozen(&env, 3, &caller, 300);

        assert_eq!(env.events().all().events().len(), 3);
    }
}
