//! Offer-accepted event — emitted after an NFT owner accepts a marketplace
//! offer (issue #927).
//!
//! Resolves issue #927: emit an `"ofr_acc"` event when a marketplace offer is
//! accepted so off-chain indexers and marketplaces can track completed offers
//! without scanning storage.
//!
//! # Event topic
//! `"ofr_acc"` — 7 characters, within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`OfferAcceptedEvent`] — offer ID, token ID, buyer, seller, accepted
//! amount, and ledger timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::marketplace::types::OfferAcceptedEvent;
use crate::types::TokenId;

/// Emit the `"ofr_acc"` event after an offer is accepted.
///
/// Must be called **after** the offer has been accepted and ownership
/// transferred, so receiving the event guarantees the acceptance is reflected
/// on-chain.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `offer_id`       — Unique identifier of the accepted offer.
/// * `token_id`       — On-chain token ID the offer targets.
/// * `buyer`          — Address of the buyer (offerer).
/// * `seller`         — Address of the seller (acceptor).
/// * `accepted_amount`— Amount the offer was accepted for, in stroops.
/// * `timestamp`      — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_offer_accepted(
    env: &Env,
    offer_id: u64,
    token_id: TokenId,
    buyer: &Address,
    seller: &Address,
    accepted_amount: i128,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("ofr_acc"),),
        OfferAcceptedEvent {
            offer_id,
            token_id,
            buyer: buyer.clone(),
            seller: seller.clone(),
            accepted_amount,
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
    fn emit_offer_accepted_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let buyer = Address::generate(&env);
            let seller = Address::generate(&env);
            emit_offer_accepted(&env, 1, 7, &buyer, &seller, 1_000, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_offer_accepted_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let buyer = Address::generate(&env);
            let seller = Address::generate(&env);
            emit_offer_accepted(&env, 42, 77, &buyer, &seller, 5_000, 1_720_000_000);
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
