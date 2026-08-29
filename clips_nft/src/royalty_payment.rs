//! Royalty payment endpoint.
//!
//! Distributes calculated royalty amounts to configured recipients when a
//! secondary sale occurs. Handles recipient / asset validation, multi-recipient
//! splits, replay protection, payment history recording, cumulative earnings
//! tracking, and event emission.

use soroban_sdk::{token, xdr::ToXdr, Address, BytesN, Env, IntoVal, String, Val, Vec};

use crate::{
    platform_fee, royalty_asset_validator, royalty_earnings, royalty_emergency, royalty_history,
    royalty_payment_replay, royalty_recipient_validator, safe_math, token_storage,
    transaction_deduction_validator,
    types::{Error, RoyaltyInfo, RoyaltyPaidEvent, RoyaltyPayment, RoyaltyPaymentResult, TokenId},
};

/// Topic label emitted with every [`RoyaltyPaidEvent`].
const ROYALTY_PAID_TOPIC: &str = "royalty_paid";

/// Processes a royalty payment for a secondary sale (issues #809, #810, #831, #832, #833, #837).
///
/// Computes the royalty amount(s) using the sale price and the token's configured
/// royalty recipients, validates the recipient(s) and asset, transfers the
/// appropriate asset from the payer to each recipient, records the payments,
/// increments cumulative earnings (token- and creator-level), enforces replay
/// protection, and emits a [`RoyaltyPaidEvent`].
///
/// # Errors
/// - [`Error::TokenNotFound`] — token does not exist or has no royalty config.
/// - [`Error::InvalidSalePrice`] — `sale_price` ≤ 0.
/// - [`Error::PaymentAlreadyProcessed`] — the same payment was already processed.
/// - [`Error::UnsupportedAsset`] — the royalty asset is not supported.
/// - [`Error::InvalidRecipient`] — a configured recipient is not a valid wallet.
/// - [`Error::RoyaltyOverflow`] — combined basis points overflow.
/// - [`Error::TotalDeductionsExceedSalePrice`] — total deductions exceed 100%.
pub fn pay_royalty(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> Result<RoyaltyPaymentResult, Error> {
    royalty_emergency::require_payments_enabled(env)?;

    if sale_price <= 0 {
        return Err(Error::InvalidSalePrice);
    }

    // 1. Load royalty configuration (fails with TokenNotFound if absent).
    let royalty = token_storage::get_royalty(env, token_id)?;

    // 2. Replay protection — generate a deterministic payment identifier and
    //    reject duplicates (issue #837).
    mark_replay(env, payer, token_id, sale_price)?;

    // 3. Validate configured recipients (issue #831) and asset (issue #810).
    royalty_recipient_validator::validate_royalty_recipients(env, &royalty)?;
    royalty_asset_validator::validate_royalty_asset(env, &royalty.asset_address)?;

    // 4. Sum the configured royalty basis points.
    let mut total_royalty_bps: u32 = 0;
    for r in royalty.recipients.iter() {
        total_royalty_bps = total_royalty_bps
            .checked_add(r.basis_points)
            .ok_or(Error::RoyaltyOverflow)?;
    }

    // 5. Enforce that combined royalty + platform fee deductions stay ≤ 100%.
    let platform_fee_bps = platform_fee::get_platform_fee(env);
    transaction_deduction_validator::validate_total_deduction_bps(total_royalty_bps, platform_fee_bps)?;

    let platform_fee_amount = safe_math::safe_royalty_amount(sale_price, platform_fee_bps)?;

    // 6. Zero-royalty short-circuit (issue #801)
    //
    // A token with a zero royalty rate owes nothing to any recipient. Return a
    // zero result immediately — skipping the distribution loop, royalty-history
    // writes, and event emissions — so no unnecessary payment operation runs.
    if total_royalty_bps == 0 {
        return Ok(RoyaltyPaymentResult {
            total_royalty: 0,
            platform_fee: platform_fee_amount,
            payments: Vec::new(env),
        });
    }

    // 7. Distribute royalty to each recipient and record the payment.
    let timestamp = env.ledger().timestamp();
    let mut payments = Vec::new(env);
    let mut total_royalty: i128 = 0;

    for recipient_cfg in royalty.recipients.iter() {
        let amount = safe_math::safe_royalty_amount(sale_price, recipient_cfg.basis_points)?;

        if amount > 0 {
            // Execute the transfer.
            if let Some(asset) = &royalty.asset_address {
                let token_client = token::Client::new(env, asset);
                token_client.transfer(payer, &recipient_cfg.recipient, &amount);
            } else {
                // Native XLM royalties must specify a transferable asset address.
                return Err(Error::InvalidConfig);
            }

            // Record a per-token history entry (issue #833).
            royalty_history::record_royalty_payment(
                env,
                token_id,
                recipient_cfg.recipient.clone(),
                amount,
                timestamp,
            );

            // Increment cumulative token earnings (issue #835).
            royalty_earnings::increment_earnings(env, token_id, amount)?;
            // Increment cumulative creator earnings (issue #834).
            royalty_earnings::increment_creator_earnings(env, &recipient_cfg.recipient, amount)?;

            // Emit a royalty-paid event (issue #836).
            env.events().publish(
                (
                    ROYALTY_PAID_TOPIC,
                    token_id,
                    recipient_cfg.recipient.clone(),
                    amount,
                ),
                RoyaltyPaidEvent {
                    token_id,
                    payer: payer.clone(),
                    receiver: recipient_cfg.recipient.clone(),
                    amount,
                    asset_address: royalty.asset_address.clone(),
                    // `pay_royalty` is the generic payout path and is not tied to a
                    // specific listing or offer, so there is no sale reference to
                    // carry. Marketplace flows emit their own event with one set.
                    sale_reference: String::from_str(env, ""),
                    sale_reference: soroban_sdk::String::from_str(env, ""),
                    timestamp,
                },
            );

            payments.push_back(RoyaltyPayment {
                token_id,
                recipient: recipient_cfg.recipient.clone(),
                amount,
                timestamp,
            });
            total_royalty = total_royalty.saturating_add(amount);
        }
    }

    Ok(RoyaltyPaymentResult {
        total_royalty,
        platform_fee: platform_fee_amount,
        payments,
    })
}

/// Generate and store the payment identifier used for replay protection.
fn mark_replay(
    env: &Env,
    payer: &Address,
    token_id: TokenId,
    sale_price: i128,
) -> Result<(), Error> {
    let mut tuple: Vec<Val> = Vec::new(env);
    tuple.push_back(payer.into_val(env));
    tuple.push_back(token_id.into_val(env));
    tuple.push_back(sale_price.into_val(env));
    let payment_id: BytesN<32> = env.crypto().sha256(&tuple.to_xdr(env)).into();
    royalty_payment_replay::mark_payment_processed(env, &payment_id)
}

/// Calculate royalty info without executing a payment (read-only preview).
///
/// Returns a summary of what `pay_royalty` would produce for the given
/// token and sale price.
pub fn royalty_info(env: &Env, token_id: TokenId, sale_price: i128) -> Result<RoyaltyInfo, Error> {
    let royalty = token_storage::get_royalty(env, token_id)?;

    let mut total_bps: u32 = 0;
    for r in royalty.recipients.iter() {
        total_bps = total_bps
            .checked_add(r.basis_points)
            .ok_or(Error::RoyaltyOverflow)?;
    }

    let royalty_amount = safe_math::safe_royalty_amount(sale_price, total_bps)?;

    let receiver = royalty
        .recipients
        .get(0)
        .map(|r| r.recipient.clone())
        .ok_or(Error::CorruptedStorage)?;

    Ok(RoyaltyInfo {
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
    fn pay_royalty_zero_bps_distributes_nothing() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 2, 0);
            assert!(pay_royalty(env, &payer, 2, 1_000_000).is_ok());
        });
    }

    #[test]
    fn pay_royalty_rejects_unsupported_asset() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 1, 500);
            // A positive royalty with no transferable asset cannot be paid.
            assert_eq!(
                pay_royalty(env, &payer, 1, 1_000_000),
                Err(Error::InvalidConfig)
            );
        });
    }

    #[test]
    fn pay_royalty_invalid_sale_price() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 4, 500);
            assert_eq!(
                pay_royalty(env, &payer, 4, 0),
                Err(Error::InvalidSalePrice)
            );
            assert_eq!(
                pay_royalty(env, &payer, 4, -100),
                Err(Error::InvalidSalePrice)
            );
        });
    }

    #[test]
    fn pay_royalty_token_not_found() {
        with_contract(|env| {
            let payer = Address::generate(env);
            assert_eq!(
                pay_royalty(env, &payer, 999, 1_000_000),
                Err(Error::TokenNotFound)
            );
        });
    }

    #[test]
    fn pay_royalty_rejects_duplicate_payment() {
        with_contract(|env| {
            let payer = Address::generate(env);
            setup_token_royalty(env, 30, 0);
            assert!(pay_royalty(env, &payer, 30, 1_000_000).is_ok());
            assert_eq!(
                pay_royalty(env, &payer, 30, 1_000_000),
                Err(Error::PaymentAlreadyProcessed)
            );
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
            assert_eq!(
                royalty_info(env, 999, 1_000_000),
                Err(Error::TokenNotFound)
            );
        });
    }
}
