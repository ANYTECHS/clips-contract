//! Reusable event helper for financial amounts and asset information.
//!
//! Provides a consistent [`AmountInfo`] struct and serialization helpers
//! so that every financial event (royalty payment, sale, offer, listing)
//! carries the same shape of amount data without copy-pasting fields.
//!
//! # Design
//!
//! `AmountInfo` is deliberately kept small and composable: it captures the
//! amount, the asset contract address, the sender, and the recipient.
//! Event structs embed it as a field rather than inheriting from it, which
//! keeps the contract type flat and avoids Soroban serialization surprises.

use soroban_sdk::{contracttype, Address};

/// Standard financial-amount payload embedded in events.
///
/// Every event that moves value (sales, royalty payments, offers) should
/// include an `AmountInfo` so indexers have a uniform shape to parse.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmountInfo {
    /// Amount in the smallest unit (stroops for native, base units for
    /// other assets). Negative values are forbidden at the call site.
    pub amount: i128,
    /// Contract address of the payment asset. For native XLM use the
    /// well-known native asset address.
    pub asset: Address,
    /// Address that initiated the transfer (buyer, payer, offerer).
    pub sender: Address,
    /// Address that receives the funds (seller, royalty recipient).
    pub recipient: Address,
}

/// Build an [`AmountInfo`] value.
///
/// This is a pure constructor — it does not publish an event. Use it
/// inside `emit_*` helpers to assemble the payload before calling
/// `env.events().publish(...)`.
pub fn build_amount_info(amount: i128, asset: &Address, sender: &Address, recipient: &Address) -> AmountInfo {
    AmountInfo {
        amount,
        asset: asset.clone(),
        sender: sender.clone(),
        recipient: recipient.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn build_amount_info_stores_all_fields() {
        let env = Env::default();
        let asset = Address::generate(&env);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let info = build_amount_info(1_000_000, &asset, &sender, &recipient);
        assert_eq!(info.amount, 1_000_000);
        assert_eq!(info.asset, asset);
        assert_eq!(info.sender, sender);
        assert_eq!(info.recipient, recipient);
    }

    #[test]
    fn amount_info_clone_is_equal() {
        let env = Env::default();
        let asset = Address::generate(&env);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let info = build_amount_info(500, &asset, &sender, &recipient);
        let cloned = info.clone();
        assert_eq!(info, cloned);
    }

    #[test]
    fn zero_amount_is_valid() {
        let env = Env::default();
        let asset = Address::generate(&env);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let info = build_amount_info(0, &asset, &sender, &recipient);
        assert_eq!(info.amount, 0);
    }

    #[test]
    fn large_amount_stored_correctly() {
        let env = Env::default();
        let asset = Address::generate(&env);
        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let info = build_amount_info(i128::MAX / 2, &asset, &sender, &recipient);
        assert_eq!(info.amount, i128::MAX / 2);
    }
}
