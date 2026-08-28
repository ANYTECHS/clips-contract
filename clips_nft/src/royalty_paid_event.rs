//! Royalty-paid event — emitted whenever creator royalties are successfully
//! distributed (issue #928).
//!
//! Resolves issue #928: emit a `"ryl_paid"` event after royalty payments are
//! distributed so off-chain indexers and creators can track royalty payouts
//! without scanning storage.
//!
//! # Event topic
//! `"ryl_paid"` — 8 characters, within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`RoyaltyPaidEvent`] — token ID, payer, recipient, amount, asset, sale
//! reference, and ledger timestamp.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{RoyaltyPaidEvent, TokenId};

/// Emit the `"ryl_paid"` event after a successful royalty distribution.
///
/// Must be called **after** all royalty transfers and history writes have
/// completed, so receiving the event guarantees the payout is reflected
/// on-chain.
///
/// # Arguments
/// * `env`            — Contract execution environment.
/// * `token_id`       — On-chain token ID the royalty was paid for.
/// * `payer`          — Address that funded the royalty payment.
/// * `recipient`      — Address that received the royalty (creator/recipient).
/// * `amount`         — Royalty amount in stroops.
/// * `asset`          — Address of the payment asset contract (`None` for
///                      native XLM).
/// * `sale_reference` — Reference linking this payout to the originating sale
///                      (e.g. listing or offer id).
/// * `timestamp`      — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_royalty_paid(
    env: &Env,
    token_id: TokenId,
    payer: &Address,
    recipient: &Address,
    amount: i128,
    asset: &Option<Address>,
    sale_reference: &String,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("ryl_paid"),),
        RoyaltyPaidEvent {
            token_id,
            payer: payer.clone(),
            receiver: recipient.clone(),
            amount,
            asset_address: asset.clone(),
            sale_reference: sale_reference.clone(),
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
        Address, Env, String,
    };

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        (env, contract_id)
    }

    #[test]
    fn emit_royalty_paid_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let payer = Address::generate(&env);
            let recipient = Address::generate(&env);
            let reference = String::from_str(&env, "listing-1");
            emit_royalty_paid(
                &env,
                7,
                &payer,
                &recipient,
                500,
                &None,
                &reference,
                1_700_000_000,
            );
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_royalty_paid_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let payer = Address::generate(&env);
            let recipient = Address::generate(&env);
            let asset = Address::generate(&env);
            let reference = String::from_str(&env, "offer-42");
            emit_royalty_paid(
                &env,
                77,
                &payer,
                &recipient,
                5_000,
                &Some(asset),
                &reference,
                1_720_000_000,
            );
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
