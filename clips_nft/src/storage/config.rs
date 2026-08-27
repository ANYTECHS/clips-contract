//! Config storage helpers — resolves issue #492.

use soroban_sdk::{panic_with_error, Env};

use crate::{
    storage::keys::StorageKey,
    types::{Config, Error},
};

const MAX_BPS: u32 = 10_000;

/// Read the stored [`Config`]. Panics with [`Error::NotInitialized`] if absent.
pub fn get_config(env: &Env) -> Config {
    if let Some(cfg) = env.storage().instance().get(&StorageKey::Config) {
        cfg
    } else {
        panic_with_error!(env, Error::NotInitialized)
    }
}

/// Persist a [`Config`] to instance storage.
pub fn set_config(env: &Env, config: &Config) {
    env.storage().instance().set(&StorageKey::Config, config);
}

/// Validate config values. Returns `Err(InvalidBasisPoints)` on bad inputs.
pub fn validate_config(config: &Config) -> Result<(), Error> {
    if config.max_royalty_bps > MAX_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    if config.platform_fee_bps > MAX_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    // Validate combined royalty + platform fee don't exceed 100%
    crate::transaction_deduction_validator::validate_total_deduction_bps(
        config.max_royalty_bps,
        config.platform_fee_bps,
    )?;
    Ok(())
}

// ─── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Config;
    use crate::AtomicMintContract;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn with_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    #[test]
    #[should_panic]
    fn get_config_panics_when_unset() {
        with_contract(|env| {
            // Should panic via `panic_with_error!(Error::NotInitialized)`
            let _ = get_config(env);
        });
    }

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

    #[test]
    fn validate_config_rejects_large_bps() {
        let env = Env::default();
        let mut cfg = Config {
            admin: Address::generate(&env),
            max_royalty_bps: 1_000,
            mint_cooldown_secs: 0,
            platform_fee_bps: 2_000,
        };
        cfg.max_royalty_bps = 20_000;
        assert_eq!(validate_config(&cfg), Err(Error::InvalidBasisPoints));

        cfg.max_royalty_bps = 1_000;
        cfg.platform_fee_bps = 50_000;
        assert_eq!(validate_config(&cfg), Err(Error::InvalidBasisPoints));
    }
}
