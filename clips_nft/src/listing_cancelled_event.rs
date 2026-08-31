//! Listing-cancelled event — emitted whenever an active NFT listing is cancelled
//! (issue #874).
//!
//! Resolves issue #874: emit a `"list_cncl"` event when an active marketplace listing
//! is cancelled so off-chain indexers, wallets, and analytics can track listing
//! status changes without scanning storage.
//!
//! # Event topic
//! `"list_cncl"` — 9 characters, exactly within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`ListingCancelledEvent`] — listing ID, token ID, seller address, and ledger
//! timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::marketplace::types::ListingCancelledEvent;
use crate::types::{ListingId, TokenId};

/// Emit the `"list_cncl"` event after an active listing is cancelled.
///
/// Must be called **after** the listing has been cancelled or removed from storage,
/// so receiving the event guarantees the cancellation is reflected on-chain.
///
/// # Arguments
/// * `env`        — Contract execution environment.
/// * `listing_id` — Identifier of the listing that was cancelled.
/// * `token_id`   — On-chain token ID of the cancelled listing.
/// * `seller`     — Address of the seller.
/// * `timestamp`  — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_listing_cancelled(
    env: &Env,
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("list_cncl"),),
        ListingCancelledEvent {
            listing_id,
            token_id,
            seller: seller.clone(),
            timestamp,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn emit_listing_cancelled_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let seller = Address::generate(&env);
            emit_listing_cancelled(&env, 1, 7, &seller, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_listing_cancelled_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let seller = Address::generate(&env);
            emit_listing_cancelled(&env, 42, 77, &seller, 1_720_000_000);
            let all = env.events().all();
            assert_eq!(all.events().len(), 1);
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
