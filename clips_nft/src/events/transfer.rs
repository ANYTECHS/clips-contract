//! NFT transferred event — centralized helper for ownership changes (issue #958).
//!
//! Mirrors `crate::transfer_event` but lives under the centralized
//! `events::transfer` namespace so callers can use `events::transfer::emit_nft_transferred`
//! without importing the standalone module. The standalone `transfer_event`
//! module is retained for backward compatibility and is re-exported here.
//!
//! # Event topic
//! Published under the short symbol `"nft_xfer"` so indexers can filter
//! it independently of the legacy `"transfer"` topic.
//!
//! # Acceptance criteria (#958)
//! - `token_id`       — On-chain token identifier that changed hands.
//! - `previous_owner` — Address that owned the token before the transfer.
//! - `new_owner`      — Address that received ownership.
//! - `timestamp`      — Ledger timestamp (seconds since Unix epoch) at transfer time.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{NFTTransferredEvent, TokenId};

/// Emit the `"nft_xfer"` event after NFT ownership has changed.
///
/// Call this **after** all storage writes are complete so receivers are
/// guaranteed the new owner is fully persisted on-chain when they
/// process the event.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `token_id`       — On-chain token ID whose ownership changed.
/// * `previous_owner` — Address that owned the token before the transfer.
/// * `new_owner`      — Address that received the token.
/// * `timestamp`      — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_transferred(
    env: &Env,
    token_id: TokenId,
    previous_owner: &Address,
    new_owner: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_xfer"),),
        NFTTransferredEvent {
            token_id,
            previous_owner: previous_owner.clone(),
            new_owner: new_owner.clone(),
            timestamp,
        },
    );
}

/// Build the event payload without publishing it.
///
/// Used by tests to verify every required field is populated correctly
/// without relying on XDR deserialization of the event log.
pub fn build_nft_transferred_event(
    env: &Env,
    token_id: TokenId,
    previous_owner: &Address,
    new_owner: &Address,
    timestamp: u64,
) -> NFTTransferredEvent {
    let _ = env;
    NFTTransferredEvent {
        token_id,
        previous_owner: previous_owner.clone(),
        new_owner: new_owner.clone(),
        timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
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
    fn emit_publishes_exactly_one_event() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            emit_nft_transferred(env, 1, &from, &to, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn payload_contains_all_four_fields() {
        with_contract(|env| {
            let from = Address::generate(env);
            let to = Address::generate(env);
            let ts: u64 = 1_720_000_000;
            let payload = build_nft_transferred_event(env, 42, &from, &to, ts);
            assert_eq!(payload.token_id, 42);
            assert_eq!(payload.previous_owner, from);
            assert_eq!(payload.new_owner, to);
            assert_eq!(payload.timestamp, ts);
        });
    }
}
