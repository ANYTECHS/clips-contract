//! Event counter storage — resolves issue #530.
//!
//! Tracks how many times each type of contract event has been emitted.
//! Event types are identified by a `u32` ID (callers define their own enum).
//!
//! # Storage
//! Key: `DataKey::EventCounter(event_type_id)` (instance storage)

use soroban_sdk::Env;

use crate::types::DataKey;

/// Increment the emission counter for `event_type_id` by one and return the new count.
pub fn increment_event_counter(env: &Env, event_type_id: u32) -> u32 {
    let count = get_event_counter(env, event_type_id) + 1;
    env.storage()
        .instance()
        .set(&DataKey::EventCounter(event_type_id), &count);
    count
}

/// Return the current emission count for `event_type_id` (0 if never incremented).
pub fn get_event_counter(env: &Env, event_type_id: u32) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::EventCounter(event_type_id))
        .unwrap_or(0)
}
