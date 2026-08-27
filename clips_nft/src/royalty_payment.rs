//! Royalty payment endpoint.

use soroban_sdk::{token, xdr::ToXdr, Address, BytesN, Env, IntoVal, Vec, Val};

use crate::{
    royalty_earnings, royalty_history, royalty_payment_replay, royalty_storage, safe_math,
    types::{Error, RoyaltyPaidEvent, TokenId},
};

/// Processes a royalty payment for a secondary sale.
///
/// Computes the royalty amount using the sale price and the token's configured
/// royalty percentage, transfers the appropriate asset from the payer to the
/// royalty recipient, and records the payment history.
///
/// Replay protection is enforced by checking and storing `payment_id`.
pub fn pay_royalty(
    env: &Env,
    payer: Address,
    token_id: TokenId,
    sale_price: i128,
) -> Result<(), Error> {
    payer.require_auth();

    // 1. Generate payment identifier
    let mut tuple: Vec<Val> = Vec::new(env);
    tuple.push_back(payer.into_val(env));
    tuple.push_back(token_id.into_val(env));
    tuple.push_back(sale_price.into_val(env));
    let payment_id = env.crypto().sha256(&tuple.to_xdr(env)).into();

    // 2. Replay protection
    royalty_payment_replay::mark_payment_processed(env, &payment_id)?;

    // 2. Load royalty configuration
    let royalty = royalty_storage::get_royalty(env, token_id)?;

    // 3. Compute payment amount
    let amount = safe_math::safe_royalty_amount(sale_price, royalty.basis_points)?;

    // 4. Transfer funds
    if amount > 0 {
        if let Some(asset) = &royalty.asset_address {
            let token_client = token::Client::new(env, asset);
            token_client.transfer(&payer, &royalty.recipient, &amount);
        } else {
            return Err(Error::InvalidConfig); // Native XLM royalties must specify an asset address
        }
    }

    // 5. Record history and cumulative earnings
    let timestamp = env.ledger().timestamp();
    royalty_history::record_royalty_payment(
        env,
        token_id,
        royalty.recipient.clone(),
        amount,
        timestamp,
    );
    royalty_earnings::increment_earnings(env, token_id, amount)?;

    // 6. Emit event
    env.events().publish(
        ("royalty", token_id, royalty.recipient.clone(), amount, royalty.asset_address.clone()),
        RoyaltyPaidEvent {
            token_id,
            payer,
            receiver: royalty.recipient,
            amount,
            asset_address: royalty.asset_address,
            timestamp,
        },
    );

    Ok(())
}
