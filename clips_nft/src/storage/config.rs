//! Contract configuration persistence.
//!
//! The [`Config`](crate::types::Config) record lives in instance storage under
//! [`StorageKey::Config`].  Writes go through `config_guard` at the entry
//! point; this module owns only the raw bytes.
//!
//! # Storage
//! Key:    [`StorageKey::Config`] (instance storage)
//! Value:  [`crate::types::Config`]
//!
//! # Operations
//! - [`get_config`]      — read the stored configuration (panics if absent)
//! - [`set_config`]      — persist a configuration snapshot
//! - [`validate_config`] — validate without writing storage

use soroban_sdk::{panic_with_error, Env};

use crate::{
    storage::keys::StorageKey,
    types::{Config, Error},
};

/// Basis-point ceiling (100 %).
const MAX_BPS: u32 = 10_000;

/// Read the stored [`Config`].
///
/// # Panics
/// Panics with [`Error::NotInitialized`] if the contract has never been
/// initialized (i.e. `set_config` was never called).
pub fn get_config(env: &Env) -> Config {
    if let Some(cfg) = env.storage().instance().get(&StorageKey::Config) {
        cfg
    } else {
        panic_with_error!(env, Error::NotInitialized)
    }
}

/// Persist a [`Config`] snapshot to instance storage.
pub fn set_config(env: &Env, config: &Config) {
    env.storage()
        .instance()
        .set(&StorageKey::Config, config);
}

/// Validate configuration values without touching storage.
///
/// # Errors
/// Returns [`Error::InvalidBasisPoints`] when `max_royalty_bps` or
/// `platform_fee_bps` exceed 10 000, or when their combined sum would exceed
/// 100 % (validated by
/// [`transaction_deduction_validator::validate_total_deduction_bps`]).
pub fn validate_config(config: &Config) -> Result<(), Error> {
    if config.max_royalty_bps > MAX_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if config.platform_fee_bps > MAX_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    crate::transaction_deduction_validator::validate_total_deduction_bps(
        config.max_royalty_bps,
        config.platform_fee_bps,
    )?;
    Ok(())
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let id = env.register(AtomicMintContract, ());
        env.as_contract(&id, || f(&env))
    }
    #[ignore]
    #[test]
    #[should_panic]
    fn get_config_panics_when_unset() {
        with_contract(|env| {
            let _ = get_config(env);
        });
    }
    #[ignore]
    #[test]
    fn set_and_get_config_roundtrip() {
        with_contract(|env| {
            let admin = Address::generate(env);
            let cfg = Config {
                admin: admin.clone(),
                max_royalty_bps: 500,
                mint_cooldown_secs: 10,
                platform_fee_bps: 100,
            };
            set_config(env, &cfg);
            let got = get_config(env);
            assert_eq!(got.admin, admin);
            assert_eq!(got.max_royalty_bps, 500);
            assert_eq!(got.mint_cooldown_secs, 10);
            assert_eq!(got.platform_fee_bps, 100);
        });
    }
    #[ignore]
    #[test]
    fn validate_config_accepts_valid_values() {
        let env = Env::default();
        let cfg = Config {
            admin: Address::generate(&env),
            max_royalty_bps: 1_000,
            mint_cooldown_secs: 0,
            platform_fee_bps: 2_000,
        };
        assert!(validate_config(&cfg).is_ok());
    }
    #[ignore]
    #[test]
    fn validate_config_rejects_royalty_above_max() {
        let env = Env::default();
        let cfg = Config {
            admin: Address::generate(&env),
            max_royalty_bps: 20_000,
            mint_cooldown_secs: 0,
            platform_fee_bps: 0,
        };
        assert_eq!(validate_config(&cfg), Err(Error::InvalidBasisPoints));
    }
    #[ignore]
    #[test]
    fn validate_config_rejects_platform_fee_above_max() {
        let env = Env::default();
        let cfg = Config {
            admin: Address::generate(&env),
            max_royalty_bps: 0,
            mint_cooldown_secs: 0,
            platform_fee_bps: 50_000,
        };
        assert_eq!(validate_config(&cfg), Err(Error::InvalidBasisPoints));
    }
}
