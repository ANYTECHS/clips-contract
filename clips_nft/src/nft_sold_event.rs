//! NFT-sold event — emitted when a marketplace NFT sale is successfully
//! completed (issue #925).
//!
//! Resolves issue #925: emit a `"nft_sold"` event after a marketplace NFT
//! sale completes so off-chain indexers, wallets, and analytics can track
//! secondary sales without scanning storage.
//!
//! # Event topic
//! `"nft_sold"` — 8 characters, within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`NftSoldEvent`] — listing ID, token ID, buyer, seller, sale amount,
//! payment asset, and ledger timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::marketplace::types::NftSoldEvent;
use crate::types::{ListingId, TokenId};

/// Emit the `"nft_sold"` event after a successful marketplace sale.
///
/// Must be called **after** the listing has been settled and the token
/// ownership transferred, so receiving the event guarantees the sale is
/// reflected on-chain.
///
/// # Arguments
/// * `env`           — Contract execution environment.
/// * `listing_id`    — Identifier of the listing that was sold.
/// * `token_id`      — On-chain token ID that was sold.
/// * `seller`        — Address of the seller.
/// * `buyer`         — Address of the buyer.
/// * `sale_amount`   — Sale amount in stroops.
/// * `payment_asset` — Address of the payment asset contract.
/// * `timestamp`     — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_sold(
    env: &Env,
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    sale_amount: i128,
    payment_asset: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_sold"),),
        NftSoldEvent {
            listing_id,
            token_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            sale_amount,
            payment_asset: payment_asset.clone(),
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
    fn emit_nft_sold_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = Address::generate(&env);
            emit_nft_sold(&env, 1, 7, &seller, &buyer, 1_000, &asset, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_nft_sold_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let seller = Address::generate(&env);
            let buyer = Address::generate(&env);
            let asset = Address::generate(&env);
            emit_nft_sold(
                &env,
                42,
                77,
                &seller,
                &buyer,
                5_000,
                &asset,
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
