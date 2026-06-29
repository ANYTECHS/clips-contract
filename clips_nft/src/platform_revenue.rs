//! Platform revenue storage.
//!
//! Tracks cumulative platform revenue generated from fees.
//!
//! # Storage
//! Key: `DataKey::PlatformRevenue` (instance storage)

use soroban_sdk::Env;

use crate::types::DataKey;

/// Overwrite the stored platform revenue with `amount`.
pub fn save_platform_revenue(env: &Env, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::PlatformRevenue, &amount);
}

/// Add `delta` to the current platform revenue total.
pub fn update_platform_revenue(env: &Env, delta: i128) {
    let current = get_platform_revenue(env);
    env.storage()
        .instance()
        .set(&DataKey::PlatformRevenue, &(current + delta));
}

/// Return the current cumulative platform revenue (defaults to `0`).
pub fn get_platform_revenue(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::PlatformRevenue)
        .unwrap_or(0)
}
