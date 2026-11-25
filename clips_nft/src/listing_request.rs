use soroban_sdk::{contracttype, Address};

use crate::types::{ListingId, ListingStatus, TokenId};

/// Lightweight request struct used when creating a new marketplace listing.
///
/// Contains only the fields the caller must provide. Serialization is
/// supported via `#[contracttype]` so this can be passed through the Soroban
/// ABI and persisted to storage if necessary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateListingRequest {
    /// On-chain token identifier to list.
    pub token_id: TokenId,
    /// Asking price in the payment asset's smallest unit.
    pub price: i128,
    /// Accepted payment asset contract address.
    pub payment_asset: Address,
    /// Unix expiration timestamp; `0` means never expires.
    pub expiration: u64,
    /// Address of the seller creating the listing.
    pub seller: Address,
}

/// Describes an NFT marketplace listing stored on-chain.
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

    #[test]
    fn create_listing_request_serializes() {
        let env = Env::default();
        let req = CreateListingRequest {
            token_id: 42,
            price: 1_000,
            payment_asset: Address::generate(&env),
            expiration: 0,
            seller: Address::generate(&env),
        };

        // Ensure serialization via XDR works and produces non-empty bytes.
        assert!(!req.to_xdr(&env).is_empty());
    }
}
