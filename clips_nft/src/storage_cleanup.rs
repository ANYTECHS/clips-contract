//! Storage cleanup helper — resolves issue #533.
//!
//! Provides reusable utilities for removing stale storage entries and detecting
//! orphaned records that lack a corresponding `Token` entry.
//!
//! # What counts as orphaned?
//! Any per-token persistent key (Metadata, Royalty, Creator, etc.) whose
//! `DataKey::Token(token_id)` entry is absent is considered orphaned.

use soroban_sdk::Env;

use crate::types::{DataKey, TokenId};

/// Remove all persistent storage entries associated with `token_id`.
///
/// Clears: `Token`, `Metadata`, `Royalty`, `Creator`, `FrozenToken`,
/// `RoyaltyHistory`, `RoyaltyRecipient`, `TokenUri`, `Approval`, `TokenClipId`.
pub fn remove_token_records(env: &Env, token_id: TokenId) {
    let storage = env.storage().persistent();
    storage.remove(&DataKey::Token(token_id));
    storage.remove(&DataKey::Metadata(token_id));
    storage.remove(&DataKey::Royalty(token_id));
    storage.remove(&DataKey::Creator(token_id));
    storage.remove(&DataKey::FrozenToken(token_id));
    storage.remove(&DataKey::RoyaltyHistory(token_id));
    storage.remove(&DataKey::RoyaltyRecipient(token_id));
    storage.remove(&DataKey::TokenUri(token_id));
    storage.remove(&DataKey::Approval(token_id));
    storage.remove(&DataKey::TokenClipId(token_id));
}

/// Return `true` if any auxiliary record exists for `token_id` but the primary
/// `Token` entry is absent — indicating an orphaned / leaked storage slot.
pub fn is_record_orphaned(env: &Env, token_id: TokenId) -> bool {
    if env.storage().persistent().has(&DataKey::Token(token_id)) {
        return false;
    }
    let storage = env.storage().persistent();
    storage.has(&DataKey::Metadata(token_id))
        || storage.has(&DataKey::Royalty(token_id))
        || storage.has(&DataKey::Creator(token_id))
        || storage.has(&DataKey::FrozenToken(token_id))
        || storage.has(&DataKey::RoyaltyHistory(token_id))
        || storage.has(&DataKey::RoyaltyRecipient(token_id))
        || storage.has(&DataKey::TokenUri(token_id))
        || storage.has(&DataKey::Approval(token_id))
        || storage.has(&DataKey::TokenClipId(token_id))
}
