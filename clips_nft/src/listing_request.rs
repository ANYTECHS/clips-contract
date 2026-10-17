use soroban_sdk::{contracttype, Address};

use crate::types::{ListingId, ListingStatus, TokenId};

/// Describes an NFT marketplace listing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingRequest {
    /// Identifier assigned to this listing at creation time.
    pub listing_id: ListingId,
    pub token_id: TokenId,
    pub price: i128,
    pub payment_asset: Address,
    pub expiration: u64,
    pub seller: Address,
    /// Current lifecycle state of the listing.
    pub status: ListingStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, xdr::ToXdr, Env};
    #[ignore]
    #[test]
    fn listing_request_serializes() {
        let env = Env::default();
        let request = ListingRequest {
            listing_id: 1,
            token_id: 1,
            price: 100,
            payment_asset: Address::generate(&env),
            expiration: 1_700_000_000,
            seller: Address::generate(&env),
            status: ListingStatus::Active,
        };

        assert!(!request.to_xdr(&env).is_empty());
    }
}
