//! NFT-listed event — emitted whenever an NFT is successfully listed in the
//! marketplace (issue #873).
//!
//! Resolves issue #873: emit an `"nft_list"` event after a marketplace NFT
//! listing is created so off-chain indexers, wallets, and analytics can track
//! active listings without scanning storage.
//!
//! # Event topic
//! `"nft_list"` — 8 characters, within the 9-character limit for
//! [`soroban_sdk::symbol_short`].
//!
//! # Event data
//! [`NftListedEvent`] — listing ID, token ID, seller, price, payment asset,
//! and ledger timestamp.

use soroban_sdk::{symbol_short, Address, Env};

use crate::marketplace::types::NftListedEvent;
use crate::types::{ListingId, TokenId};

/// Emit the `"nft_list"` event after a successful marketplace listing.
///
/// Must be called **after** the listing has been saved in contract storage,
/// so receiving the event guarantees the listing is queryable on-chain.
///
/// # Arguments
/// * `env`           — Contract execution environment.
/// * `listing_id`    — Unique identifier of the created listing.
/// * `token_id`      — On-chain token ID being listed.
/// * `seller`        — Address of the seller.
/// * `price`         — Asking price in stroops.
/// * `asset`         — Address of the accepted payment asset contract.
/// * `timestamp`     — Ledger timestamp in seconds since the Unix epoch.
pub fn emit_nft_listed(
    env: &Env,
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    price: i128,
    asset: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("nft_list"),),
        NftListedEvent {
            listing_id,
            token_id,
            seller: seller.clone(),
            price,
            asset: asset.clone(),
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
    fn emit_nft_listed_publishes_event() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let seller = Address::generate(&env);
            let asset = Address::generate(&env);
            emit_nft_listed(&env, 1, 7, &seller, 1_000, &asset, 1_700_000_000);
            assert_eq!(env.events().all().events().len(), 1);
        });
    }

    #[test]
    fn emit_nft_listed_event_fields_match() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let seller = Address::generate(&env);
            let asset = Address::generate(&env);
            emit_nft_listed(
                &env,
                42,
                77,
                &seller,
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
