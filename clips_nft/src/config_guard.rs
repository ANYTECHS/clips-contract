//! Configuration update guard (Issue #485).
//!
//! Restricts configuration updates to authorized administrators and validates
//! that proposed configuration values are structurally sound before they are
//! persisted. Provides reusable guards that validate caller identity and
//! configuration state in a single, composable call.
//!
//! # Two-tier design
//!
//! - [`require_config_admin`] — authorization only (verifies the caller is the
//!   contract admin and has signed). Used by any entry point that needs admin
//!   checks without a config payload.
//! - [`guard_config_update`] — authorization **plus** validation. Verifies the
//!   caller is admin, then validates every field of the supplied [`Config`]
//!   before permitting the write. This is the recommended guard for all
//!   administrative configuration changes.
//!
//! # Usage
//!
//! ```rust,ignore
//! config_guard::guard_config_update(&env, &caller, &new_config)?;
//! // If Ok, the caller is authorized AND the config is valid.
//! storage::config::set_config(&env, &new_config);
//! ```
//!
//! # Validation rules
//!
//! | Field | Rule | Error on violation |
//! |-------|------|--------------------|
//! | `max_royalty_bps` | Must be ≤ `MAX_ROYALTY_BPS` (10 000) | `InvalidBasisPoints` |
//! | `platform_fee_bps` | Must be ≤ `MAX_PLATFORM_FEE_BPS` (1 000) | `InvalidFee` |
//! | Combined `max_royalty_bps + platform_fee_bps` | Must be ≤ 10 000 | `TotalDeductionsExceedSalePrice` |
//! | `admin` | Must be a valid Starknet/Soroban address | *(type-checked by Soroban)* |
//! | `mint_cooldown_secs` | Any `u64` is accepted (0 = no cooldown) | — |

use soroban_sdk::{Address, Env};

use crate::storage_constants::{MAX_PLATFORM_FEE_BPS, MAX_ROYALTY_BPS};
use crate::transaction_deduction_validator::validate_total_deduction_bps;
use crate::types::{DataKey, Error};
use crate::types::Config;

/// Validate that the caller is the contract owner/admin and require auth.
///
/// This is the single entry point for all configuration update authorization.
/// It checks:
/// 1. The contract has been initialized (admin exists).
/// 2. The caller matches the stored admin address.
/// 3. The caller has signed the invocation (`require_auth`).
///
/// # Errors
/// - [`Error::NotInitialized`] if the contract has not been initialized.
/// - [`Error::UnauthorizedConfigurationUpdate`] if the caller is not admin.
pub fn require_config_admin(env: &Env, caller: &Address) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    if *caller != admin {
        return Err(Error::UnauthorizedConfigurationUpdate);
    }
    caller.require_auth();
    Ok(())
}

/// Validate a [`Config`] struct's field values without requiring authorization.
///
/// This function checks only the **values** — it does not inspect any on-chain
/// state or verify caller identity. Use [`guard_config_update`] when you need
/// both authorization and validation; use this function when you already have
/// an authorized caller and want to validate the proposed values before writing.
///
/// # Validation rules
///
/// 1. `platform_fee_bps` must be ≤ [`MAX_PLATFORM_FEE_BPS`] (1 000).
/// 2. `max_royalty_bps` must be ≤ [`MAX_ROYALTY_BPS`] (10 000).
/// 3. The combined `max_royalty_bps + platform_fee_bps` must be ≤ 10 000,
///    enforced via [`validate_total_deduction_bps`].
///
/// # Errors
/// - [`Error::InvalidFee`] — `platform_fee_bps` exceeds the maximum.
/// - [`Error::InvalidBasisPoints`] — `max_royalty_bps` exceeds the maximum.
/// - [`Error::TotalDeductionsExceedSalePrice`] — combined bps exceed 100%.
pub fn validate_config(config: &Config) -> Result<(), Error> {
    if config.platform_fee_bps > MAX_PLATFORM_FEE_BPS {
        return Err(Error::InvalidFee);
    }
    if config.max_royalty_bps > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }
    validate_total_deduction_bps(config.max_royalty_bps, config.platform_fee_bps)?;
    Ok(())
}

/// Guard a full configuration update: authorize the caller **and** validate the
/// proposed configuration values.
///
/// This is the recommended single-call guard for any entry point that accepts a
/// new [`Config`] and persists it. It enforces:
///
/// 1. **Authorization** — the caller must be the stored admin and must have
///    signed (`require_auth`).
/// 2. **Initialization** — the contract must already be initialized.
/// 3. **Value validation** — every numeric field is range-checked and the
///    combined royalty + fee total is verified to not exceed 100%.
///
/// # Arguments
///
/// * `env`      — The Soroban execution environment.
/// * `caller`   — The address attempting the configuration update.
/// * `config`   — The proposed new configuration to validate.
///
/// # Errors
///
/// | Error | Trigger |
/// |-------|---------|
/// | `NotInitialized` | Contract has no admin set. |
/// | `UnauthorizedConfigurationUpdate` | `caller` is not the stored admin. |
/// | `InvalidFee` | `config.platform_fee_bps` exceeds `MAX_PLATFORM_FEE_BPS`. |
/// | `InvalidBasisPoints` | `config.max_royalty_bps` exceeds `MAX_ROYALTY_BPS`. |
/// | `TotalDeductionsExceedSalePrice` | Combined bps exceed 10 000. |
pub fn guard_config_update(
    env: &Env,
    caller: &Address,
    config: &Config,
) -> Result<(), Error> {
    require_config_admin(env, caller)?;
    validate_config(config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Config;
    use soroban_sdk::{testutils::Address as _, Env};

    // ─── Test helpers ───────────────────────────────────────────────────────────

    fn make_admin(env: &Env) -> Address {
        let admin = Address::generate(env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        admin
    }

    fn valid_config(admin: &Address) -> Config {
        Config {
            admin: admin.clone(),
            max_royalty_bps: 500,
            mint_cooldown_secs: 60,
            platform_fee_bps: 100,
        }
    }

    // ─── require_config_admin tests ─────────────────────────────────────────────

    #[test]
    fn require_config_admin_passes_when_admin() {
        let env = Env::default();
        let admin = make_admin(&env);

        assert!(require_config_admin(&env, &admin).is_ok());
    }

    #[test]
    fn require_config_admin_rejects_non_admin() {
        let env = Env::default();
        let _admin = make_admin(&env);
        let intruder = Address::generate(&env);

        assert_eq!(
            require_config_admin(&env, &intruder),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    }

    #[test]
    fn require_config_admin_rejects_when_not_initialized() {
        let env = Env::default();
        let caller = Address::generate(&env);

        assert_eq!(
            require_config_admin(&env, &caller),
            Err(Error::NotInitialized)
        );
    }

    // ─── validate_config tests ──────────────────────────────────────────────────

    #[test]
    fn validate_config_accepts_valid_values() {
        let admin = Address::generate(&Env::default());
        let config = valid_config(&admin);

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_accepts_zero_fees() {
        let admin = Address::generate(&Env::default());
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 0,
            mint_cooldown_secs: 0,
            platform_fee_bps: 0,
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_accepts_max_individual_values() {
        let admin = Address::generate(&Env::default());
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: MAX_ROYALTY_BPS,
            mint_cooldown_secs: 0,
            platform_fee_bps: 0,
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_rejects_high_platform_fee() {
        let admin = Address::generate(&Env::default());
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 500,
            mint_cooldown_secs: 0,
            platform_fee_bps: MAX_PLATFORM_FEE_BPS + 1,
        };

        assert_eq!(validate_config(&config), Err(Error::InvalidFee));
    }

    #[test]
    fn validate_config_rejects_high_royalty_bps() {
        let admin = Address::generate(&Env::default());
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: MAX_ROYALTY_BPS + 1,
            mint_cooldown_secs: 0,
            platform_fee_bps: 100,
        };

        assert_eq!(validate_config(&config), Err(Error::InvalidBasisPoints));
    }

    #[test]
    fn validate_config_rejects_combined_total_exceeding_100_percent() {
        let admin = Address::generate(&Env::default());
        // 9 500 bps royalty + 1 000 bps fee = 10 500 > 10 000
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 9_500,
            mint_cooldown_secs: 0,
            platform_fee_bps: 1_000,
        };

        assert_eq!(
            validate_config(&config),
            Err(Error::TotalDeductionsExceedSalePrice)
        );
    }

    #[test]
    fn validate_config_accepts_combined_total_at_boundary() {
        let admin = Address::generate(&Env::default());
        // 9 000 bps royalty + 1 000 bps fee = 10 000 (= 100%)
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 9_000,
            mint_cooldown_secs: 0,
            platform_fee_bps: 1_000,
        };

        assert!(validate_config(&config).is_ok());
    }

    // ─── guard_config_update tests ──────────────────────────────────────────────

    #[test]
    fn guard_config_update_passes_for_admin_with_valid_config() {
        let env = Env::default();
        let admin = make_admin(&env);
        let config = valid_config(&admin);

        assert!(guard_config_update(&env, &admin, &config).is_ok());
    }

    #[test]
    fn guard_config_update_rejects_non_admin_even_with_valid_config() {
        let env = Env::default();
        let admin = make_admin(&env);
        let intruder = Address::generate(&env);
        let config = valid_config(&admin);

        assert_eq!(
            guard_config_update(&env, &intruder, &config),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    }

    #[test]
    fn guard_config_update_rejects_admin_with_invalid_fee() {
        let env = Env::default();
        let admin = make_admin(&env);
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 500,
            mint_cooldown_secs: 0,
            platform_fee_bps: MAX_PLATFORM_FEE_BPS + 1,
        };

        assert_eq!(
            guard_config_update(&env, &admin, &config),
            Err(Error::InvalidFee)
        );
    }

    #[test]
    fn guard_config_update_rejects_admin_with_invalid_royalty() {
        let env = Env::default();
        let admin = make_admin(&env);
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: MAX_ROYALTY_BPS + 1,
            mint_cooldown_secs: 0,
            platform_fee_bps: 100,
        };

        assert_eq!(
            guard_config_update(&env, &admin, &config),
            Err(Error::InvalidBasisPoints)
        );
    }

    #[test]
    fn guard_config_update_rejects_admin_with_excessive_combined_bps() {
        let env = Env::default();
        let admin = make_admin(&env);
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 10_000,
            mint_cooldown_secs: 0,
            platform_fee_bps: 1,
        };

        assert_eq!(
            guard_config_update(&env, &admin, &config),
            Err(Error::TotalDeductionsExceedSalePrice)
        );
    }

    #[test]
    fn guard_config_update_rejects_uninitialized_contract() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let config = valid_config(&caller);

        assert_eq!(
            guard_config_update(&env, &caller, &config),
            Err(Error::NotInitialized)
        );
    }

    #[test]
    fn guard_config_update_auth_runs_before_validation() {
        let env = Env::default();
        let admin = make_admin(&env);
        let intruder = Address::generate(&env);
        // Config has invalid values, but auth should fail first.
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: MAX_ROYALTY_BPS + 1,
            mint_cooldown_secs: 0,
            platform_fee_bps: MAX_PLATFORM_FEE_BPS + 1,
        };

        assert_eq!(
            guard_config_update(&env, &intruder, &config),
            Err(Error::UnauthorizedConfigurationUpdate)
        );
    }

    #[test]
    fn guard_config_update_accepts_max_royalty_with_zero_fee() {
        let env = Env::default();
        let admin = make_admin(&env);
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: MAX_ROYALTY_BPS,
            mint_cooldown_secs: 3600,
            platform_fee_bps: 0,
        };

        assert!(guard_config_update(&env, &admin, &config).is_ok());
    }

    #[test]
    fn guard_config_update_accepts_max_fee_with_zero_royalty() {
        let env = Env::default();
        let admin = make_admin(&env);
        let config = Config {
            admin: admin.clone(),
            max_royalty_bps: 0,
            mint_cooldown_secs: 0,
            platform_fee_bps: MAX_PLATFORM_FEE_BPS,
        };

        assert!(guard_config_update(&env, &admin, &config).is_ok());
    }
}
