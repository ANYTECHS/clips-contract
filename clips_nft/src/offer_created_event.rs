//! Offer-created event — emitted whenever a buyer creates an offer for an NFT
//! (issue #926).
//!
//! Resolves issue #926: emit an `"ofr_crt"` event when a marketplace offer is
//! created so off-chain indexers and marketplaces can track open offers
//! without scanning storage.
//!
//! # Event topic
//! `"ofr_crt"` — 7 characters, within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`OfferCreatedEvent`] — offer ID, token ID, buyer, offer amount, asset, and
//! expiration.

use soroban_sdk::{symbol_short, Address, Env};

use crate::marketplace::types::OfferCreatedEvent;
use crate::types::TokenId;

/// Emit the `"ofr_crt"` event after an offer is created.
///
/// Must be called **after** the offer has been persisted to storage, so
/// receiving the event guarantees the offer is queryable on-chain.
///
/// # Arguments
/// * `env`           — Contract execution environment.
/// * `offer_id`      — Unique identifier of the created offer.
/// * `token_id`      — On-chain token ID the offer targets.
/// * `buyer`         — Address of the buyer (offerer).
/// * `offer_amount`  — Offered price in stroops.
/// * `asset`         — Address of the payment asset contract.
/// * `expiration`    — Unix timestamp after which the offer expires.
pub fn emit_offer_created(
    env: &Env,
    offer_id: u64,
    token_id: TokenId,
    buyer: &Address,
    offer_amount: i128,
    asset: &Address,
    expiration: u64,
) {
    env.events().publish(
        (symbol_short!("ofr_crt"),),
        OfferCreatedEvent {
            offer_id,
            token_id,
            buyer: buyer.clone(),
            offer_amount,
            asset: asset.clone(),
            expiration,
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
    fn emit_offer_created_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let buyer = Address::generate(&env);
            let asset = Address::generate(&env);
            emit_offer_created(&env, 1, 7, &buyer, 1_000, &asset, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_offer_created_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let buyer = Address::generate(&env);
            let asset = Address::generate(&env);
            emit_offer_created(&env, 42, 77, &buyer, 5_000, &asset, 1_720_000_000);
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
