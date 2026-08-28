//! Royalty payment function (issue #809).
//!
//! Distributes calculated royalty amounts to configured recipients when a
//! secondary sale occurs. Handles multi-recipient splits, asset validation,
//! platform fee collection, and payment history recording.

use soroban_sdk::{Address, Env, Vec};

use crate::platform_fee;
use crate::platform_recipient;
use crate::platform_revenue;
use crate::royalty_asset_validator;
use crate::royalty_history;
use crate::safe_math;
use crate::token_storage;
use crate::types::{Error, RoyaltyPaidEvent, RoyaltyPayment, RoyaltyPaymentResult, TokenId};

/// Topic label emitted with every [`RoyaltyPaidEvent`].
const ROYALTY_PAID_TOPIC: &str = "royalty_paid";

/// Execute a royalty payment for a token secondary sale.
///
/// # Arguments
/// * `payer` — Address paying the royalty (typically the buyer or marketplace).
/// * `token_id` — On-chain token identifier.
/// * `sale_price` — Sale price in the asset's smallest unit (must be > 0).
///
/// # Returns
/// [`RoyaltyPaymentResult`] with the total royalty, platform fee, and per-recipient payment records.
///
/// # Errors
/// - [`Error::TokenNotFound`] — token does not exist or has no royalty config.
/// - [`Error::InvalidSalePrice`] — `sale_price` ≤ 0.
/// - [`Error::UnsupportedAsset`] — the royalty asset is not in the supported list.
/// - [`Error::RoyaltyOverflow`] — arithmetic overflow during calculation.
/// - [`Error::TotalDeductionsExceedSalePrice`] — combined royalty + fee > sale price.
pub fn pay_royalty(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> Result<RoyaltyPaymentResult, Error> {
    // 1. Read royalty config
    let royalty = token_storage::get_royalty(env, token_id)?;

    // 2. Validate asset is supported
    royalty_asset_validator::validate_royalty_asset(env, &royalty.asset_address)?;

    // 3. Calculate total royalty basis points
    let mut total_royalty_bps: u32 = 0;
    for i in 0..royalty.recipients.len() {
        if let Some(r) = royalty.recipients.get(i) {
            total_royalty_bps = total_royalty_bps
                .checked_add(r.basis_points)
                .ok_or(Error::RoyaltyOverflow)?;
        }
    }

    // 4. Calculate platform fee
    let platform_fee_bps = platform_fee::get_platform_fee(env);

    // 5. Validate total deductions don't exceed sale price
    let (total_royalty_amount, platform_fee_amount) =
        crate::transaction_deduction_validator::validate_total_deduction_amount(
            sale_price,
            total_royalty_bps,
            platform_fee_bps,
        )?;

    // 6. Distribute royalty to each recipient
    let timestamp = env.ledger().timestamp();
    let mut payments: Vec<RoyaltyPayment> = Vec::new(env);

    for i in 0..royalty.recipients.len() {
        if let Some(recipient_config) = royalty.recipients.get(i) {
            let amount = safe_math::safe_royalty_amount(sale_price, recipient_config.basis_points)?;

            if amount > 0 {
                // Record payment in history
                royalty_history::record_royalty_payment(
                    env,
                    token_id,
                    recipient_config.recipient.clone(),
                    amount,
                    timestamp,
                );

                // Emit event
                env.events().publish(
                    (ROYALTY_PAID_TOPIC,),
                    RoyaltyPaidEvent {
                        token_id,
                        payer: payer.clone(),
                        receiver: recipient_config.recipient.clone(),
                        amount,
                        asset_address: royalty.asset_address.clone(),
                    },
                );

                payments.push_back(RoyaltyPayment {
                    token_id,
                    recipient: recipient_config.recipient.clone(),
                    amount,
                    timestamp,
                });
            }
        }
    }

    // 7. Record platform fee if applicable
    if platform_fee_amount > 0 {
        if let Ok(platform_wallet) = platform_recipient::get_platform_recipient(env) {
            platform_revenue::update_platform_revenue(env, platform_fee_amount);
        }
    }

    Ok(RoyaltyPaymentResult {
        total_royalty: total_royalty_amount,
        platform_fee: platform_fee_amount,
        payments,
    })
}

/// Calculate royalty info without executing a payment (read-only preview).
///
/// Returns a summary of what `pay_royalty` would produce for the given
/// token and sale price.
pub fn royalty_info(
    env: &Env,
    token_id: TokenId,
    sale_price: i128,
) -> Result<crate::types::RoyaltyInfo, Error> {
    let royalty = token_storage::get_royalty(env, token_id)?;

    let mut total_bps: u32 = 0;
    for i in 0..royalty.recipients.len() {
        if let Some(r) = royalty.recipients.get(i) {
            total_bps = total_bps
                .checked_add(r.basis_points)
                .ok_or(Error::RoyaltyOverflow)?;
        }
    }

    let royalty_amount = safe_math::safe_royalty_amount(sale_price, total_bps)?;

    let receiver = royalty
        .recipients
        .get(0)
        .map(|r| r.recipient.clone())
        .ok_or(Error::CorruptedStorage)?;

    Ok(crate::types::RoyaltyInfo {
        receiver,
        royalty_amount,
        asset_address: royalty.asset_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Royalty, RoyaltyRecipient};
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

    fn setup_token_royalty(env: &Env, token_id: TokenId, bps: u32) -> Address {
        let recipient = Address::generate(env);
        let mut recipients = soroban_sdk::Vec::new(env);
        recipients.push_back(RoyaltyRecipient {
            recipient: recipient.clone(),
            basis_points: bps,
        });
        let royalty = Royalty {
            recipients,
            asset_address: None,
        };
        token_storage::set_royalty(env, token_id, &royalty);
        recipient
    }

    #[test]
    fn pay_royalty_native_xlm() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let expected_recipient = setup_token_royalty(env, 1, 500);

            let result = pay_royalty(env, &payer, 1, 1_000_000).unwrap();
            assert_eq!(result.total_royalty, 50_000); // 5% of 1_000_000
            assert_eq!(result.payments.len(), 1);
            assert_eq!(result.payments.get(0).unwrap().recipient, expected_recipient);
            assert_eq!(result.payments.get(0).unwrap().amount, 50_000);
        });
    }

    #[test]
    fn pay_royalty_zero_bps() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 2, 0);

            let result = pay_royalty(env, &payer, 2, 1_000_000).unwrap();
            assert_eq!(result.total_royalty, 0);
            assert_eq!(result.payments.len(), 0);
        });
    }

    #[test]
    fn pay_royalty_full_100_percent() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 3, 10_000);

            let result = pay_royalty(env, &payer, 3, 1_000_000).unwrap();
            assert_eq!(result.total_royalty, 1_000_000);
        });
    }

    #[test]
    fn pay_royalty_invalid_sale_price() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 4, 500);

            assert_eq!(pay_royalty(env, &payer, 4, 0), Err(Error::InvalidSalePrice));
            assert_eq!(pay_royalty(env, &payer, 4, -100), Err(Error::InvalidSalePrice));
        });
    }

    #[test]
    fn pay_royalty_token_not_found() {
        with_contract(|env| {
            let payer = Address::generate(env);
            assert_eq!(pay_royalty(env, &payer, 999, 1_000_000), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn royalty_info_returns_correct_amount() {
        with_contract(|env| {
            let recipient = setup_token_royalty(env, 10, 500);
            let info = royalty_info(env, 10, 1_000_000).unwrap();
            assert_eq!(info.receiver, recipient);
            assert_eq!(info.royalty_amount, 50_000);
        });
    }

    #[test]
    fn royalty_info_token_not_found() {
        with_contract(|env| {
            assert_eq!(royalty_info(env, 999, 1_000_000), Err(Error::TokenNotFound));
        });
    }

    #[test]
    fn pay_royalty_with_unsupported_asset() {
        with_contract(|env| {
            let payer = Address::generate(env);
            let unsupported_asset = Address::generate(env);
            let mut recipients = soroban_sdk::Vec::new(env);
            recipients.push_back(RoyaltyRecipient {
                recipient: Address::generate(env),
                basis_points: 500,
            });
            let royalty = Royalty {
                recipients,
                asset_address: Some(unsupported_asset),
            };
            token_storage::set_royalty(env, 20, &royalty);

            assert_eq!(
                pay_royalty(env, &payer, 20, 1_000_000),
                Err(Error::UnsupportedAsset)
            );
        });
    }
}
