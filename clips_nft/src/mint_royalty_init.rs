//! Initialize NFT royalty information at mint time (issue #670).
//!
//! Assigns royalty recipient and percentage when an NFT is created. Callers may
//! supply explicit values via the mint request; when either field is absent the
//! contract-wide defaults are applied (`DefaultRoyaltyBps` and the provided
//! fallback recipient, typically the NFT owner/creator).
//!
//! # Storage
//! - `DataKey::Royalty(token_id)` → full [`Royalty`] blob
//! - `DataKey::RoyaltyRecipient(token_id)` → recipient address
//! - `DataKey::RoyaltyPercentage(token_id)` → basis points
//!
//! # Validation
//! Recipient is checked via [`crate::royalty_recipient_validator`] before any
//! write (#671). Basis points are checked against [`MAX_ROYALTY_BPS`].

use soroban_sdk::{Address, Env};

use crate::default_royalty::{get_default_royalty_bps, MAX_ROYALTY_BPS};
use crate::royalty_percentage;
use crate::royalty_recipient_validator::validate_royalty_recipient;
use crate::types::{DataKey, Error, Royalty, TokenId};

/// Optional royalty fields supplied with a mint request.
///
/// Either field may be omitted; missing values fall back to contract defaults.
#[derive(Clone)]
pub struct RoyaltyInitParams {
    /// Royalty recipient override. When `None`, `fallback_recipient` is used.
    pub recipient: Option<Address>,
    /// Royalty percentage in basis points. When `None`, the contract default is used.
    pub basis_points: Option<u32>,
    /// Optional asset address for non-native royalty payments.
    pub asset_address: Option<Address>,
}

/// Resolve and persist royalty information for a newly minted NFT.
///
/// # Arguments
/// * `fallback_recipient` — used when `params.recipient` is `None` (usually the
///   owner or creator of the NFT).
///
/// # Errors
/// - [`Error::InvalidRecipient`] if the resolved recipient fails validation.
/// - [`Error::InvalidBasisPoints`] if the resolved percentage exceeds the max.
pub fn initialize_nft_royalty(
    env: &Env,
    token_id: TokenId,
    params: &RoyaltyInitParams,
    fallback_recipient: &Address,
) -> Result<Royalty, Error> {
    let recipient = params
        .recipient
        .clone()
        .unwrap_or_else(|| fallback_recipient.clone());

    validate_royalty_recipient(env, &recipient)?;

    let basis_points = params
        .basis_points
        .unwrap_or_else(|| get_default_royalty_bps(env));
    if basis_points > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }

    let royalty = Royalty {
        recipient: recipient.clone(),
        basis_points,
        asset_address: params.asset_address.clone(),
    };

    // Persist the full royalty blob.
    env.storage()
        .persistent()
        .set(&DataKey::Royalty(token_id), &royalty);

    // Persist standalone recipient (#670 acceptance: save royalty recipient).
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyRecipient(token_id), &recipient);

    // Persist standalone percentage (#670 acceptance: save royalty percentage).
    royalty_percentage::set_royalty_percentage(env, token_id, basis_points)?;

    Ok(royalty)
}

/// Convenience wrapper when the mint request already carries a full [`Royalty`].
///
/// Still runs recipient + percentage validation before persisting.
pub fn initialize_nft_royalty_from_royalty(
    env: &Env,
    token_id: TokenId,
    royalty: &Royalty,
) -> Result<Royalty, Error> {
    let params = RoyaltyInitParams {
        recipient: Some(royalty.recipient.clone()),
        basis_points: Some(royalty.basis_points),
        asset_address: royalty.asset_address.clone(),
    };
    initialize_nft_royalty(env, token_id, &params, &royalty.recipient)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_royalty::{set_default_royalty_bps, DEFAULT_ROYALTY_BPS};
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
    fn saves_explicit_recipient_and_percentage() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: Some(recipient.clone()),
                basis_points: Some(750),
                asset_address: None,
            };

            let royalty = initialize_nft_royalty(env, 1, &params, &owner).unwrap();

            assert_eq!(royalty.recipient, recipient);
            assert_eq!(royalty.basis_points, 750);

            let stored: Royalty = env
                .storage()
                .persistent()
                .get(&DataKey::Royalty(1))
                .unwrap();
            assert_eq!(stored.recipient, recipient);
            assert_eq!(stored.basis_points, 750);

            let stored_recipient: Address = env
                .storage()
                .persistent()
                .get(&DataKey::RoyaltyRecipient(1))
                .unwrap();
            assert_eq!(stored_recipient, recipient);

            assert_eq!(
                royalty_percentage::get_royalty_percentage(env, 1).unwrap(),
                750
            );
        });
    }

    #[test]
    fn applies_default_bps_when_absent() {
        with_contract(|env| {
            set_default_royalty_bps(env, 300).unwrap();
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: None,
                basis_points: None,
                asset_address: None,
            };

            let royalty = initialize_nft_royalty(env, 2, &params, &owner).unwrap();

            assert_eq!(royalty.recipient, owner);
            assert_eq!(royalty.basis_points, 300);
        });
    }

    #[test]
    fn applies_builtin_default_bps_when_never_configured() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: None,
                basis_points: None,
                asset_address: None,
            };

            let royalty = initialize_nft_royalty(env, 3, &params, &owner).unwrap();
            assert_eq!(royalty.basis_points, DEFAULT_ROYALTY_BPS);
            assert_eq!(royalty.recipient, owner);
        });
    }

    #[test]
    fn rejects_invalid_recipient() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            let params = RoyaltyInitParams {
                recipient: Some(contract.clone()),
                basis_points: Some(500),
                asset_address: None,
            };

            assert_eq!(
                initialize_nft_royalty(env, 4, &params, &contract),
                Err(Error::InvalidRecipient)
            );
        });
    }

    #[test]
    fn rejects_percentage_above_limit() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipient: Some(owner.clone()),
                basis_points: Some(MAX_ROYALTY_BPS + 1),
                asset_address: None,
            };

            assert_eq!(
                initialize_nft_royalty(env, 5, &params, &owner),
                Err(Error::InvalidBasisPoints)
            );
        });
    }

    #[test]
    fn initialize_from_full_royalty_struct() {
        with_contract(|env| {
            let recipient = Address::generate(env);
            let royalty = Royalty {
                recipient: recipient.clone(),
                basis_points: 250,
                asset_address: None,
            };

            let stored = initialize_nft_royalty_from_royalty(env, 6, &royalty).unwrap();
            assert_eq!(stored.recipient, recipient);
            assert_eq!(stored.basis_points, 250);
            assert_eq!(
                royalty_percentage::get_royalty_percentage(env, 6).unwrap(),
                250
            );
        });
    }
}
