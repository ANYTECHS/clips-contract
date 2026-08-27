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

use soroban_sdk::{Address, Env, Vec};

use crate::default_royalty::{get_default_royalty_bps, MAX_ROYALTY_BPS};
use crate::royalty_percentage;
use crate::royalty_recipient_validator::validate_royalty_recipient;
use crate::types::{DataKey, Error, Royalty, RoyaltyRecipient, TokenId};

/// Optional royalty fields supplied with a mint request.
///
/// Either field may be omitted; missing values fall back to contract defaults.
#[derive(Clone)]
pub struct RoyaltyInitParams {
    /// Multi-recipient royalty overrides. When `None`, `fallback_recipient` is used
    /// as the sole recipient with the default basis points.
    pub recipients: Option<Vec<RoyaltyRecipient>>,
    /// Optional asset address for non-native royalty payments.
    pub asset_address: Option<Address>,
}

/// Resolve and persist royalty information for a newly minted NFT.
///
/// # Arguments
/// * `fallback_recipient` — used when `params.recipients` is `None` (usually the
///   owner or creator of the NFT). Becomes the sole recipient with default basis points.
///
/// # Errors
/// - [`Error::InvalidRecipient`] if any resolved recipient fails validation.
/// - [`Error::InvalidBasisPoints`] if the resolved percentage exceeds the max.
pub fn initialize_nft_royalty(
    env: &Env,
    token_id: TokenId,
    params: &RoyaltyInitParams,
    fallback_recipient: &Address,
) -> Result<Royalty, Error> {
    let recipients = match &params.recipients {
        Some(r) => r.clone(),
        None => {
            let bps = get_default_royalty_bps(env);
            soroban_sdk::vec![env, RoyaltyRecipient { recipient: fallback_recipient.clone(), basis_points: bps }]
        }
    };

    if recipients.is_empty() {
        return Err(Error::InvalidBasisPoints);
    }

    let mut total_bps: u32 = 0;
    for r in recipients.iter() {
        validate_royalty_recipient(env, &r.recipient)?;
        total_bps = total_bps.saturating_add(r.basis_points);
    }
    if total_bps > MAX_ROYALTY_BPS {
    let basis_points = params
        .basis_points
        .unwrap_or_else(|| get_default_royalty_bps(env));
    if basis_points > MAX_ROYALTY_BPS {
        return Err(Error::InvalidBasisPoints);
    }

    let royalty = Royalty {
        recipients: recipients.clone(),
        asset_address: params.asset_address.clone(),
    };

    // Persist the full royalty blob.
    env.storage()
        .persistent()
        .set(&DataKey::Royalty(token_id), &royalty);

    // Persist standalone recipient (#670 acceptance: save first royalty recipient).
    env.storage()
        .persistent()
        .set(&DataKey::RoyaltyRecipient(token_id), &recipients.get(0).unwrap().recipient);

    // Persist standalone percentage (#670 acceptance: save total royalty percentage).
    royalty_percentage::set_royalty_percentage(env, token_id, total_bps)?;

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
    let first_recipient = royalty.recipients.get(0).unwrap();
    let params = RoyaltyInitParams {
        recipients: Some(royalty.recipients.clone()),
        asset_address: royalty.asset_address.clone(),
    };
    initialize_nft_royalty(env, token_id, &params, &first_recipient.recipient)
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
                recipients: Some(soroban_sdk::vec![env, RoyaltyRecipient { recipient: recipient.clone(), basis_points: 750 }]),
                asset_address: None,
            };

            let royalty = initialize_nft_royalty(env, 1, &params, &owner).unwrap();

            assert_eq!(royalty.recipients.get(0).unwrap().recipient, recipient);
            assert_eq!(royalty.recipients.get(0).unwrap().basis_points, 750);

            let stored: Royalty = env
                .storage()
                .persistent()
                .get(&DataKey::Royalty(1))
                .unwrap();
            assert_eq!(stored.recipients.get(0).unwrap().recipient, recipient);
            assert_eq!(stored.recipients.get(0).unwrap().basis_points, 750);

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
                recipients: None,
                asset_address: None,
            };

            let royalty = initialize_nft_royalty(env, 2, &params, &owner).unwrap();

            assert_eq!(royalty.recipients.get(0).unwrap().recipient, owner);
            assert_eq!(royalty.recipients.get(0).unwrap().basis_points, 300);
        });
    }

    #[test]
    fn applies_builtin_default_bps_when_never_configured() {
        with_contract(|env| {
            let owner = Address::generate(env);
            let params = RoyaltyInitParams {
                recipients: None,
                asset_address: None,
            };

            let royalty = initialize_nft_royalty(env, 3, &params, &owner).unwrap();
            assert_eq!(royalty.recipients.get(0).unwrap().basis_points, DEFAULT_ROYALTY_BPS);
            assert_eq!(royalty.recipients.get(0).unwrap().recipient, owner);
        });
    }

    #[test]
    fn rejects_invalid_recipient() {
        with_contract(|env| {
            let contract = env.current_contract_address();
            let params = RoyaltyInitParams {
                recipients: Some(soroban_sdk::vec![env, RoyaltyRecipient { recipient: contract.clone(), basis_points: 500 }]),
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
                recipients: Some(soroban_sdk::vec![env, RoyaltyRecipient { recipient: owner.clone(), basis_points: MAX_ROYALTY_BPS + 1 }]),
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
                recipients: soroban_sdk::vec![env, RoyaltyRecipient { recipient: recipient.clone(), basis_points: 250 }],
                asset_address: None,
            };

            let stored = initialize_nft_royalty_from_royalty(env, 6, &royalty).unwrap();
            assert_eq!(stored.recipients.get(0).unwrap().recipient, recipient);
            assert_eq!(stored.recipients.get(0).unwrap().basis_points, 250);
            assert_eq!(
                royalty_percentage::get_royalty_percentage(env, 6).unwrap(),
                250
            );
        });
    }
}
