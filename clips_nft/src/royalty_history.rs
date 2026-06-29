//! Royalty payment history storage.
//!
//! Tracks royalty payment records per token. Each entry captures the
//! recipient, amount, and ledger timestamp at the time of payment.
//!
//! # Storage
//! Key: `DataKey::RoyaltyHistory(token_id)` (persistent storage)

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, RoyaltyPayment, TokenId};

/// Append a royalty payment record for the given token.
pub fn record_royalty_payment(
    env: &Env,
    token_id: TokenId,
    recipient: Address,
    amount: i128,
    timestamp: u64,
) {
    let mut history = get_royalty_history(env, token_id);
    history.push_back(RoyaltyPayment {
        token_id,
        recipient,
        amount,
        timestamp,
    });
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyHistory(token_id), &history);
}

/// Return all royalty payment records for a token (empty if none recorded).
pub fn get_royalty_history(env: &Env, token_id: TokenId) -> Vec<RoyaltyPayment> {
    env.storage()
        .persistent()
        .get(&DataKey::RoyaltyHistory(token_id))
        .unwrap_or_else(|| Vec::new(env))
}
