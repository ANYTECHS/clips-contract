//! Royalty-assigned event — emitted when royalty is successfully assigned during minting.
//!
//! Resolves issue #695: emit a `"ryl_asgn"` event every time royalty
//! information is persisted as part of the mint pipeline.  Off-chain indexers
//! and marketplaces can subscribe to this event to track royalty configurations
//! at creation time without needing additional storage reads.
//!
//! # Event topic
//! `"ryl_asgn"` — 8 characters, well within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`RoyaltyAssignedEvent`] — token ID, recipient address, basis points, and ledger
//! timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::types::{RoyaltyAssignedEvent, TokenId};

/// Emit the `"ryl_asgn"` event after royalty has been persisted for a token.
///
/// Must be called **after** all royalty storage writes have completed so that
/// receiving the event guarantees the royalty is already queryable on-chain.
///
/// # Arguments
/// * `env`          — Contract execution environment.
/// * `token_id`     — On-chain token ID the royalty was assigned to.
/// * `recipient`    — Address that will receive royalty payments.
/// * `basis_points` — Royalty percentage in basis points (0–10 000).
/// * `timestamp`    — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_royalty_assigned(
    env: &Env,
    token_id: TokenId,
    recipient: &Address,
    basis_points: u32,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("ryl_asgn"),),
        RoyaltyAssignedEvent {
            token_id,
            recipient: recipient.clone(),
            basis_points,
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoyaltyAssignedEvent;
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Events},
        Address, Env,
    };

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    #[test]
    fn emit_royalty_assigned_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            emit_royalty_assigned(&env, 1, &recipient, 500, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_royalty_assigned_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            let token_id: TokenId = 42;
            let basis_points: u32 = 750;
            let timestamp: u64 = 1_720_000_000;

            emit_royalty_assigned(&env, token_id, &recipient, basis_points, timestamp);

            let all = env.events().all();
            assert_eq!(all.events().len(), 1);

            let (_, data): (soroban_sdk::Vec<soroban_sdk::Val>, RoyaltyAssignedEvent) =
                all.events().get(0).unwrap();
            assert_eq!(data.token_id, token_id);
            assert_eq!(data.recipient, recipient);
            assert_eq!(data.basis_points, basis_points);
            assert_eq!(data.timestamp, timestamp);
        });
    }

    #[test]
    fn emit_royalty_assigned_zero_bps_is_valid() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            // 0 bps (no royalty) is a legitimate value and should still emit.
            emit_royalty_assigned(&env, 5, &recipient, 0, 1_700_000_001);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_royalty_assigned_max_bps_is_valid() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            // 10_000 bps = 100 %; already validated upstream, just verify emission.
            emit_royalty_assigned(&env, 99, &recipient, 10_000, 1_700_000_002);
            let all = env.events().all();
            assert_eq!(all.events().len(), 1);
            let (_, data): (soroban_sdk::Vec<soroban_sdk::Val>, RoyaltyAssignedEvent) =
                all.events().get(0).unwrap();
            assert_eq!(data.basis_points, 10_000);
        });
    }

    #[test]
    fn no_event_emitted_without_calling_function() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            // Sanity check: no spurious events appear on a clean environment.
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
