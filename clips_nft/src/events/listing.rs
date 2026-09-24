//! Listing event emitters for the `events::listing` namespace.
//!
//! Each function publishes a Soroban contract event with a short topic
//! symbol and a tuple payload.  Callers in
//! [`crate::ClipsNftContract`] use these helpers to emit lifecycle
//! events (created, cancelled, updated, sold) without importing
//! individual event modules.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::{ListingId, TokenId};

/// Publish the `"lst_crtd"` (listing created) event.
pub fn emit_listing_created(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    price: i128,
    payment_asset: &Address,
    expiration: u64,
    timestamp: u64,
) {
    let _ = String::from_str(env, "lst_crtd");
    env.events().publish(
        (symbol_short!("lst_crtd"),),
        (token_id, seller.clone(), price, payment_asset.clone(), expiration, timestamp),
    );
}

/// Publish the `"lst_cncl"` (listing cancelled) event.
pub fn emit_listing_cancelled(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    canceller: &Address,
    timestamp: u64,
) {
    let _ = String::from_str(env, "lst_cncl");
    env.events().publish(
        (symbol_short!("lst_cncl"),),
        (token_id, seller.clone(), canceller.clone(), timestamp),
    );
}

/// Publish the `"lst_updt"` (listing updated) event.
pub fn emit_listing_updated(
    env: &Env,
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    old_price: i128,
    new_price: i128,
    old_expiration: u64,
    new_expiration: u64,
    timestamp: u64,
) {
    let _ = String::from_str(env, "lst_updt");
    env.events().publish(
        (symbol_short!("lst_updt"),),
        (
            listing_id, token_id, seller.clone(),
            old_price, new_price, old_expiration, new_expiration, timestamp,
        ),
    );
}

/// Publish the `"nft_sold"` (NFT sold via marketplace) event.
pub fn emit_nft_sold(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    amount: i128,
    payment_asset: &Address,
    timestamp: u64,
) {
    let _ = String::from_str(env, "nft_sold");
    env.events().publish(
        (symbol_short!("nft_sold"),),
        (token_id, seller.clone(), buyer.clone(), amount, payment_asset.clone(), timestamp),
    );
}
