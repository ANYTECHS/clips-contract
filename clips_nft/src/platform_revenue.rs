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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::Env;

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    fn read_revenue_defaults_to_zero() {
        with_contract(|env| {
            assert_eq!(get_platform_revenue(env), 0);
        });
    }

    #[test]
    fn save_revenue_persists_amount() {
        with_contract(|env| {
            save_platform_revenue(env, 500);
            assert_eq!(get_platform_revenue(env), 500);
        });
    }

    #[test]
    fn update_revenue_accumulates() {
        with_contract(|env| {
            save_platform_revenue(env, 500);
            update_platform_revenue(env, 250);
            assert_eq!(get_platform_revenue(env), 750);
        });
    }
}
