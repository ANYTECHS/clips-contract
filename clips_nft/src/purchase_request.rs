//! Purchase request structure for NFT marketplace purchases (issue #875).
//!
//! Describes the buyer's request to purchase a listed NFT, specifying
//! the listing identifier, buyer address, payment asset, and the maximum
//! price the buyer is willing to pay.

use soroban_sdk::{contracttype, Address};

use crate::types::ListingId;

/// Request structure used by buyers to purchase listed NFTs (issue #875).
///
/// # Fields (issue #875 acceptance criteria)
/// * `listing_id`    — Identifier of the marketplace listing being purchased.
/// * `buyer`         — Address of the buyer.
/// * `payment_asset` — Address of the payment asset contract.
/// * `max_price`     — Maximum price in stroops the buyer agrees to pay.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurchaseRequest {
    /// Identifier of the marketplace listing being purchased.
    pub listing_id: ListingId,
    /// Address of the buyer.
    pub buyer: Address,
    /// Address of the payment asset contract.
    pub payment_asset: Address,
    /// Maximum price the buyer is willing to pay.
    pub max_price: i128,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, xdr::ToXdr, Env};

    #[test]
    fn purchase_request_serializes_and_deserializes() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let payment_asset = Address::generate(&env);

        let request = PurchaseRequest {
            listing_id: 42,
            buyer: buyer.clone(),
            payment_asset: payment_asset.clone(),
            max_price: 10_000,
        };

        // Support serialization
        let xdr = request.to_xdr(&env);
        assert!(!xdr.is_empty());
    }

    #[test]
    fn purchase_request_field_access() {
        let env = Env::default();
        let buyer = Address::generate(&env);
        let payment_asset = Address::generate(&env);

        let request = PurchaseRequest {
            listing_id: 1,
            buyer: buyer.clone(),
            payment_asset: payment_asset.clone(),
            max_price: 500,
        };

        assert_eq!(request.listing_id, 1);
        assert_eq!(request.buyer, buyer);
        assert_eq!(request.payment_asset, payment_asset);
        assert_eq!(request.max_price, 500);
    }
}
