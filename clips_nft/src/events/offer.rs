//! Offer event emitters for the `events::offer` namespace.
//!
//! Each function publishes a Soroban contract event with a short topic
//! symbol and a tuple payload.  Callers in
//! [`crate::ClipsNftContract`] use these helpers to emit offer lifecycle
//! events (made, accepted, cancelled) without importing individual
//! event modules.

use soroban_sdk::{symbol_short, Address, Env, String};

use crate::types::TokenId;

/// Publish the `"ofr_made"` (offer created) event.
pub fn emit_offer_made(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
    timestamp: u64,
) {
    let _ = String::from_str(env, "ofr_made");
    env.events().publish(
        (symbol_short!("ofr_made"),),
        (token_id, buyer.clone(), price, payment_asset.clone(), expires_at, timestamp),
    );
}

/// Publish the `"ofr_accpt"` (offer accepted) event.
pub fn emit_offer_accepted(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    price: i128,
    payment_asset: &Address,
    timestamp: u64,
) {
    let _ = String::from_str(env, "ofr_accpt");
    env.events().publish(
        (symbol_short!("ofr_accpt"),),
        (token_id, seller.clone(), buyer.clone(), price, payment_asset.clone(), timestamp),
    );
}

/// Publish the `"ofr_cncl"` (offer cancelled) event.
pub fn emit_offer_cancelled(
    env: &Env,
    token_id: TokenId,
    buyer: &Address,
    canceller: &Address,
    timestamp: u64,
) {
    let _ = String::from_str(env, "ofr_cncl");
    env.events().publish(
        (symbol_short!("ofr_cncl"),),
        (token_id, buyer.clone(), canceller.clone(), timestamp),
    );
}
