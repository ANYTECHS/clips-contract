//! Per-NFT royalty configuration storage (issue #783).
//!
//! Creates persistent storage associating each NFT (token ID) with its royalty
//! configuration — the wallet that receives royalty payments and the royalty
//! percentage in basis points.
//!
//! # Storage
//! Key: `DataKey::NftRoyaltyConfig(token_id)` (persistent storage)
//!
//! # Acceptance criteria
//! - **Map token ID to royalty configuration** — [`set_nft_royalty_config`]
//! - **Add getter** — [`get_nft_royalty_config`], [`has_nft_royalty_config`]
//! - **Add setter** — [`set_nft_royalty_config`]
//! - **Add storage tests** — unit tests in this module

use soroban_sdk::Env;

use crate::royalty_config::RoyaltyConfig;
use crate::types::{DataKey, Error, TokenId};

// ─── Public API ───────────────────────────────────────────────────────────────

/// Persist (create or overwrite) the royalty configuration for `token_id`.
///
/// The configuration is validated before being stored:
/// - `royalty_bps` must be within `0..=10_000` (see [`RoyaltyConfig::validate`]).
///
/// # Errors
/// - [`Error::RoyaltyTooHigh`] when `royalty_bps > 10_000`.
pub fn set_nft_royalty_config(
    env: &Env,
    token_id: TokenId,
    config: &RoyaltyConfig,
) -> Result<(), Error> {
    config.validate()?;
    env.storage()
        .persistent()
        .set(&DataKey::NftRoyaltyConfig(token_id), config);
    Ok(())
}

/// Return the stored royalty configuration for `token_id`.
///
/// # Errors
/// Returns [`Error::TokenNotFound`] if no configuration has been saved.
pub fn get_nft_royalty_config(env: &Env, token_id: TokenId) -> Result<RoyaltyConfig, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::NftRoyaltyConfig(token_id))
        .ok_or(Error::TokenNotFound)
}

/// Return `true` if a royalty configuration has been stored for `token_id`.
pub fn has_nft_royalty_config(env: &Env, token_id: TokenId) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::NftRoyaltyConfig(token_id))
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

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
        let contract_id = env.register(AtomicMintContract, ());
        env.as_contract(&contract_id, || f(&env))
    }

    fn config(env: &Env, recipient: &Address, bps: u32) -> RoyaltyConfig {
        RoyaltyConfig {
            recipient: recipient.clone(),
            royalty_bps: bps,
        }
    }

    // ── set_nft_royalty_config / get_nft_royalty_config ───────────────────────

    #[test]
    fn set_and_get_round_trip() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let cfg = config(env, &recipient, 500);
            set_nft_royalty_config(env, 1, &cfg).unwrap();

            let got = get_nft_royalty_config(env, 1).unwrap();
            assert_eq!(got.recipient, recipient);
            assert_eq!(got.royalty_bps, 500);
        });
    }

    #[test]
    fn get_missing_returns_not_found() {
        with_contract(|env| {
            assert_eq!(get_nft_royalty_config(env, 99), Err(Error::TokenNotFound));
            assert!(!has_nft_royalty_config(env, 99));
        });
    }

    #[test]
    fn has_returns_true_after_set() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            set_nft_royalty_config(env, 7, &config(env, &recipient, 250)).unwrap();
            assert!(has_nft_royalty_config(env, 7));
        });
    }

    #[test]
    fn overwrite_updates_existing_config() {
        with_contract(|env| {
            let first = Address::generate(env);
            let second = Address::generate(env);
            set_nft_royalty_config(env, 1, &config(env, &first, 250)).unwrap();
            set_nft_royalty_config(env, 1, &config(env, &second, 750)).unwrap();

            let got = get_nft_royalty_config(env, 1).unwrap();
            assert_eq!(got.recipient, second);
            assert_eq!(got.royalty_bps, 750);
        });
    }

    #[test]
    fn zero_bps_is_accepted() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            set_nft_royalty_config(env, 2, &config(env, &recipient, 0)).unwrap();
            assert_eq!(get_nft_royalty_config(env, 2).unwrap().royalty_bps, 0);
        });
    }

    #[test]
    fn bps_above_max_is_rejected() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            assert_eq!(
                set_nft_royalty_config(env, 3, &config(env, &recipient, 10_001)),
                Err(Error::RoyaltyTooHigh)
            );
            assert!(!has_nft_royalty_config(env, 3));
        });
    }

    #[test]
    fn configs_are_isolated_per_token() {
        with_contract(|env| {
            let alice = Address::generate(env);
            let bob = Address::generate(env);
            set_nft_royalty_config(env, 1, &config(env, &alice, 100)).unwrap();
            set_nft_royalty_config(env, 2, &config(env, &bob, 200)).unwrap();

            assert_eq!(get_nft_royalty_config(env, 1).unwrap().recipient, alice);
            assert_eq!(get_nft_royalty_config(env, 2).unwrap().recipient, bob);
        });
    }
}
