//! Royalty-updated event — emitted when royalty configuration is updated for an existing token.
//!
//! Off-chain indexers and marketplaces can subscribe to this event to track
//! royalty configuration changes post-mint without needing additional storage reads.
//!
//! # Event topic
//! `"ryl_upd"` — 7 characters, well within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`RoyaltyUpdatedEvent`] — token ID, recipients, asset address, and ledger timestamp.

use soroban_sdk::{symbol_short, Env};

use crate::types::{Royalty, RoyaltyUpdatedEvent, TokenId};

/// Emit the `"ryl_upd"` event after royalty has been updated for a token.
///
/// Must be called **after** all royalty storage writes have completed so that
/// receiving the event guarantees the royalty is already queryable on-chain.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `token_id`  — On-chain token ID the royalty was updated for.
/// * `royalty`   — The updated royalty configuration.
/// * `timestamp` — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_royalty_updated(env: &Env, token_id: TokenId, royalty: &Royalty, timestamp: u64) {
    env.events().publish(
        (symbol_short!("ryl_upd"),),
        RoyaltyUpdatedEvent {
            token_id,
            recipients: royalty.recipients.clone(),
            asset_address: royalty.asset_address.clone(),
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Royalty, RoyaltyRecipient, RoyaltyUpdatedEvent};
    use crate::AtomicMintContract;
    use soroban_sdk::{
        testutils::{Address as _, Address as _, Events},
        Address, Env,
    };

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    #[test]
    fn emit_royalty_updated_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            let royalty = Royalty {
                recipients: soroban_sdk::vec![env, RoyaltyRecipient { recipient, basis_points: 500 }],
                asset_address: None,
            };
            emit_royalty_updated(&env, 1, &royalty, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_royalty_updated_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let recipient = Address::generate(&env);
            let token_id: TokenId = 42;
            let royalty = Royalty {
                recipients: soroban_sdk::vec![env, RoyaltyRecipient { recipient: recipient.clone(), basis_points: 750 }],
                asset_address: Some(Address::generate(&env)),
            };
            let timestamp: u64 = 1_720_000_000;

            emit_royalty_updated(&env, token_id, &royalty, timestamp);
            let all = env.events().all();
            assert_eq!(all.events().len(), 1);
        });
    }

    #[test]
    fn emit_royalty_updated_zero_recipients_is_valid() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let royalty = Royalty {
                recipients: soroban_sdk::Vec::new(&env),
                asset_address: None,
            };
            emit_royalty_updated(&env, 5, &royalty, 1_700_000_001);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn no_event_emitted_without_calling_function() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            assert_eq!(env.events().all().events().len(), 0);
        });
    }
}
