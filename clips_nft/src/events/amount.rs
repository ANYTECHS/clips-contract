//! Financial amount and asset events (issue #911).
//!
//! Provides reusable event types and helper functions for emitting financial
//! amounts, payment asset contract references, and party identities (sender
//! and recipient). Ensures consistent representation across all financial and
//! payment events in the ClipCash contract ecosystem.
//!
//! # Topic labels
//! - `"amount"` — general financial amount / valuation event.
//! - `"amt_trns"` — financial amount transferred between sender and recipient.

use soroban_sdk::{contracttype, symbol_short, Address, Env};

/// Reusable event emitted for financial amounts and asset information (issue #911).
///
/// Captures financial valuation, payment asset, sender, recipient, and timestamp
/// to provide a uniform event schema across all payment-related workflows.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmountEvent {
    /// Financial amount in stroops / base units.
    pub amount: i128,
    /// Contract address of the payment asset.
    pub asset: Address,
    /// Address funding or initiating the financial operation.
    pub sender: Address,
    /// Address receiving the payment or beneficiary.
    pub recipient: Address,
    /// Unix timestamp of the event.
    pub timestamp: u64,
}

/// Reusable event emitted specifically when financial assets are transferred
/// between two parties (issue #911).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmountTransferredEvent {
    /// Transferred amount in stroops / base units.
    pub amount: i128,
    /// Contract address of the payment asset.
    pub asset: Address,
    /// Address transferring the amount (payer).
    pub sender: Address,
    /// Address receiving the amount (payee).
    pub recipient: Address,
    /// Unix timestamp of the transfer.
    pub timestamp: u64,
}

/// Emit an [`AmountEvent`] with topic `"amount"`.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `amount`    — Financial amount in stroops / base units.
/// * `asset`     — Payment asset contract address.
/// * `sender`    — Address funding or sending the payment.
/// * `recipient` — Address receiving the payment.
/// * `timestamp` — Unix timestamp of the event.
pub fn emit_amount(
    env: &Env,
    amount: i128,
    asset: &Address,
    sender: &Address,
    recipient: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("amount"),),
        AmountEvent {
            amount,
            asset: asset.clone(),
            sender: sender.clone(),
            recipient: recipient.clone(),
            timestamp,
        },
    );
}

/// Emit an [`AmountEvent`] using the current ledger timestamp.
pub fn emit_amount_now(
    env: &Env,
    amount: i128,
    asset: &Address,
    sender: &Address,
    recipient: &Address,
) {
    emit_amount(
        env,
        amount,
        asset,
        sender,
        recipient,
        env.ledger().timestamp(),
    );
}

/// Emit an [`AmountTransferredEvent`] with topic `"amt_trns"`.
///
/// # Arguments
/// * `env`       — Contract execution environment.
/// * `amount`    — Transferred amount in stroops / base units.
/// * `asset`     — Payment asset contract address.
/// * `sender`    — Address transferring the amount.
/// * `recipient` — Address receiving the amount.
/// * `timestamp` — Unix timestamp of the transfer.
pub fn emit_amount_transferred(
    env: &Env,
    amount: i128,
    asset: &Address,
    sender: &Address,
    recipient: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("amt_trns"),),
        AmountTransferredEvent {
            amount,
            asset: asset.clone(),
            sender: sender.clone(),
            recipient: recipient.clone(),
            timestamp,
        },
    );
}

/// Emit an [`AmountTransferredEvent`] using the current ledger timestamp.
pub fn emit_amount_transferred_now(
    env: &Env,
    amount: i128,
    asset: &Address,
    sender: &Address,
    recipient: &Address,
) {
    emit_amount_transferred(
        env,
        amount,
        asset,
        sender,
        recipient,
        env.ledger().timestamp(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipsNftContract;
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger},
        vec, IntoVal,
    };

    fn with_test_contract<F, R>(f: F) -> R
    where
        F: FnOnce(&Env, Address) -> R,
    {
        let env = Env::default();
        let contract_id = env.register(ClipsNftContract, ());
        env.as_contract(&contract_id, || f(&env, contract_id.clone()))
    }

    #[test]
    fn test_emit_amount_event_fields_and_topic() {
        with_test_contract(|env, contract_id| {
            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            let asset = Address::generate(env);
            let amount = 5_000_000i128;
            let timestamp = 1726000000u64;

            emit_amount(env, amount, &asset, &sender, &recipient, timestamp);

            let events = env.events().all();
            assert_eq!(events.len(), 1);

            let expected_event = AmountEvent {
                amount,
                asset: asset.clone(),
                sender: sender.clone(),
                recipient: recipient.clone(),
                timestamp,
            };

            assert_eq!(
                events,
                vec![
                    env,
                    (
                        contract_id,
                        (symbol_short!("amount"),).into_val(env),
                        expected_event.into_val(env),
                    )
                ]
            );
        });
    }

    #[test]
    fn test_emit_amount_now_uses_ledger_timestamp() {
        with_test_contract(|env, contract_id| {
            env.ledger().set_timestamp(1726112233);

            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            let asset = Address::generate(env);
            let amount = 10_500_000i128;

            emit_amount_now(env, amount, &asset, &sender, &recipient);

            let events = env.events().all();
            assert_eq!(events.len(), 1);

            let expected_event = AmountEvent {
                amount,
                asset: asset.clone(),
                sender: sender.clone(),
                recipient: recipient.clone(),
                timestamp: 1726112233,
            };

            assert_eq!(
                events,
                vec![
                    env,
                    (
                        contract_id,
                        (symbol_short!("amount"),).into_val(env),
                        expected_event.into_val(env),
                    )
                ]
            );
        });
    }

    #[test]
    fn test_emit_amount_transferred_fields_and_topic() {
        with_test_contract(|env, contract_id| {
            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            let asset = Address::generate(env);
            let amount = 25_000_000i128;
            let timestamp = 1726999999u64;

            emit_amount_transferred(env, amount, &asset, &sender, &recipient, timestamp);

            let events = env.events().all();
            assert_eq!(events.len(), 1);

            let expected_event = AmountTransferredEvent {
                amount,
                asset: asset.clone(),
                sender: sender.clone(),
                recipient: recipient.clone(),
                timestamp,
            };

            assert_eq!(
                events,
                vec![
                    env,
                    (
                        contract_id,
                        (symbol_short!("amt_trns"),).into_val(env),
                        expected_event.into_val(env),
                    )
                ]
            );
        });
    }

    #[test]
    fn test_emit_amount_transferred_now_uses_ledger_timestamp() {
        with_test_contract(|env, contract_id| {
            env.ledger().set_timestamp(1726554433);

            let sender = Address::generate(env);
            let recipient = Address::generate(env);
            let asset = Address::generate(env);
            let amount = 88_000_000i128;

            emit_amount_transferred_now(env, amount, &asset, &sender, &recipient);

            let events = env.events().all();
            assert_eq!(events.len(), 1);

            let expected_event = AmountTransferredEvent {
                amount,
                asset: asset.clone(),
                sender: sender.clone(),
                recipient: recipient.clone(),
                timestamp: 1726554433,
            };

            assert_eq!(
                events,
                vec![
                    env,
                    (
                        contract_id,
                        (symbol_short!("amt_trns"),).into_val(env),
                        expected_event.into_val(env),
                    )
                ]
            );
        });
    }
}
