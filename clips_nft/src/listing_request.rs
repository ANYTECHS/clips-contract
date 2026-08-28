use soroban_sdk::{contracttype, Address};

use crate::types::TokenId;

/// Describes an NFT marketplace listing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingRequest {
    pub token_id: TokenId,
    pub price: i128,
    pub payment_asset: Address,
    pub expiration: u64,
    pub seller: Address,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, xdr::ToXdr, Env};

    #[test]
    fn listing_request_serializes() {
        let env = Env::default();
        let request = ListingRequest {
            token_id: 1,
            price: 100,
            payment_asset: Address::generate(&env),
            expiration: 1_700_000_000,
            seller: Address::generate(&env),
        };

        assert!(!request.to_xdr(&env).is_empty());
    }
}
