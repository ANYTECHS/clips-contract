//! Marketplace listing and sale events.
//!
//! Defines every event emitted by the listing lifecycle and exposes a small
//! `emit_*` helper per event. All emission logic is centralized here so the
//! rest of the contract never publishes a raw topic string.
//! This module is the single source of truth for listing lifecycle payloads
//! and their short event topics.  Keeping the typed payloads here makes the
//! fields emitted by [`crate::ClipCashNFT`] stable for indexers.

use soroban_sdk::{contracttype, symbol_short, Address, Env};

use crate::types::{ListingId, TokenId};

/// Emitted when a seller creates a new marketplace listing (#862).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCreatedEvent {
    /// Token ID that was listed.
    pub token_id: TokenId,
    /// Address of the seller who created the listing.
    pub seller: Address,
    /// Asking price in stroops.
    pub price: i128,
    /// Accepted payment asset contract address.
    pub payment_asset: Address,
    /// Unix expiration timestamp (`0` = never expires).
    pub expires_at: u64,
    /// Unix timestamp of creation.
    pub timestamp: u64,
}

/// Emitted when a seller updates an active listing's price or expiration (#871).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingUpdatedEvent {
    /// Listing ID of the updated listing.
/// Emitted when a seller updates an active listing's price or expiration (#965).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingUpdatedEvent {
    /// Unique identifier of the updated listing.
    pub listing_id: ListingId,
    /// Token ID of the updated listing.
    pub token_id: TokenId,
    /// Seller who performed the update.
    pub seller: Address,
    /// Previous asking price in stroops.
    pub old_price: i128,
    /// New asking price in stroops.
    pub new_price: i128,
    /// Previous expiration timestamp.
    pub old_expires_at: u64,
    /// New expiration timestamp.
    /// Previous expiration timestamp (`0` = never expires).
    pub old_expires_at: u64,
    /// New expiration timestamp (`0` = never expires).
    pub new_expires_at: u64,
    /// Unix timestamp of the update.
    pub timestamp: u64,
}

/// Emitted when a listing is cancelled by the seller or an authorized operator (#924).
/// Emitted when a listing is cancelled by its seller (#924).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingCancelledEvent {
    /// Token ID of the cancelled listing.
    pub token_id: TokenId,
    /// Seller who originally created the listing.
    pub seller: Address,
    /// Address that performed the cancellation (may differ from `seller`).
    /// Address that performed the cancellation.
    pub cancelled_by: Address,
    /// Unix timestamp of the cancellation.
    pub timestamp: u64,
}

/// Emitted when an NFT is sold through a marketplace purchase (#884).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftSoldEvent {
    /// Token ID that was sold.
    pub token_id: TokenId,
    /// Seller who received the proceeds.
    pub seller: Address,
    /// Buyer who purchased the token.
    pub buyer: Address,
    /// Sale amount in stroops.
    pub sale_amount: i128,
    /// Payment asset contract address.
    pub payment_asset: Address,
    /// Unix timestamp of the sale.
    pub timestamp: u64,
}

/// Emit [`ListingCreatedEvent`].
/// Build the payload for a listing-created event.
pub fn build_listing_created_event(
    token_id: TokenId,
    seller: &Address,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
    timestamp: u64,
) -> ListingCreatedEvent {
    ListingCreatedEvent {
        token_id,
        seller: seller.clone(),
        price,
        payment_asset: payment_asset.clone(),
        expires_at,
        timestamp,
    }
}

/// Emit a listing-created event under the `lst_crt` topic.
pub fn emit_listing_created(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    price: i128,
    payment_asset: &Address,
    expires_at: u64,
    timestamp: u64,
) {
    env.events().publish(
        (crate::event_topics::TOPIC_LISTING,),
        ListingCreatedEvent {
            token_id,
            seller: seller.clone(),
            price,
            payment_asset: payment_asset.clone(),
            expires_at,
            timestamp,
        },
    );
}

/// Emit [`ListingUpdatedEvent`].
        build_listing_created_event(
            token_id,
            seller,
            price,
            payment_asset,
            expires_at,
            timestamp,
        ),
    );
}

/// Build the payload for a listing-updated event.
///
/// Exposing the builder keeps the previous/new values together and makes the
/// acceptance-criteria fields directly unit-testable without relying on event
/// serialization internals.
pub fn build_listing_updated_event(
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    old_price: i128,
    new_price: i128,
    old_expires_at: u64,
    new_expires_at: u64,
    timestamp: u64,
) -> ListingUpdatedEvent {
    ListingUpdatedEvent {
        listing_id,
        token_id,
        seller: seller.clone(),
        old_price,
        new_price,
        old_expires_at,
        new_expires_at,
        timestamp,
    }
}

/// Emit a listing-updated event under the `lst_upd` topic.
pub fn emit_listing_updated(
    env: &Env,
    listing_id: ListingId,
    token_id: TokenId,
    seller: &Address,
    old_price: i128,
    new_price: i128,
    old_expires_at: u64,
    new_expires_at: u64,
    timestamp: u64,
) {
    env.events().publish(
        (crate::event_topics::TOPIC_LISTING,),
        ListingUpdatedEvent {
            listing_id,
            token_id,
            seller: seller.clone(),
        build_listing_updated_event(
            listing_id,
            token_id,
            seller,
            old_price,
            new_price,
            old_expires_at,
            new_expires_at,
            timestamp,
        },
    );
}

/// Emit [`ListingCancelledEvent`].
        ),
    );
}

/// Build the payload for a listing-cancelled event.
pub fn build_listing_cancelled_event(
    token_id: TokenId,
    seller: &Address,
    cancelled_by: &Address,
    timestamp: u64,
) -> ListingCancelledEvent {
    ListingCancelledEvent {
        token_id,
        seller: seller.clone(),
        cancelled_by: cancelled_by.clone(),
        timestamp,
    }
}

/// Emit a listing-cancelled event under the `lst_can` topic.
pub fn emit_listing_cancelled(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    cancelled_by: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (crate::event_topics::TOPIC_LISTING_CANCELLED,),
        ListingCancelledEvent {
            token_id,
            seller: seller.clone(),
            cancelled_by: cancelled_by.clone(),
            timestamp,
        },
    );
}

/// Emit [`NftSoldEvent`].
        build_listing_cancelled_event(token_id, seller, cancelled_by, timestamp),
    );
}

/// Build the payload for an NFT-sold event.
pub fn build_nft_sold_event(
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    sale_amount: i128,
    payment_asset: &Address,
    timestamp: u64,
) -> NftSoldEvent {
    NftSoldEvent {
        token_id,
        seller: seller.clone(),
        buyer: buyer.clone(),
        sale_amount,
        payment_asset: payment_asset.clone(),
        timestamp,
    }
}

/// Emit an NFT-sold event under the `nft_sold` topic.
pub fn emit_nft_sold(
    env: &Env,
    token_id: TokenId,
    seller: &Address,
    buyer: &Address,
    sale_amount: i128,
    payment_asset: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (crate::event_topics::TOPIC_SALE,),
        NftSoldEvent {
            token_id,
            seller: seller.clone(),
            buyer: buyer.clone(),
            sale_amount,
            payment_asset: payment_asset.clone(),
            timestamp,
        },
    );
}
        build_nft_sold_event(
            token_id,
            seller,
            buyer,
            sale_amount,
            payment_asset,
            timestamp,
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events},
        xdr::ContractEventBody,
        Address, Env, Symbol, TryFromVal,
    };

    fn find_listing_updated_event(env: &Env) -> Option<ListingUpdatedEvent> {
        let expected_topic: Symbol = symbol_short!("lst_upd");
        for event in env.events().all().events() {
            if let ContractEventBody::V0(v0) = &event.body {
                if v0.topics.len() == 1
                    && Symbol::try_from_val(env, &v0.topics[0]) == Ok(expected_topic)
                {
                    return ListingUpdatedEvent::try_from_val(env, &v0.data).ok();
                }
            }
        }
        None
    }

    #[test]
    fn listing_updated_payload_contains_previous_and_new_values() {
        let env = Env::default();
        let seller = Address::generate(&env);

        let event =
            build_listing_updated_event(17, 42, &seller, 1_000, 2_500, 10_000, 20_000, 30_000);

        assert_eq!(event.listing_id, 17);
        assert_eq!(event.token_id, 42);
        assert_eq!(event.seller, seller);
        assert_eq!(event.old_price, 1_000);
        assert_eq!(event.new_price, 2_500);
        assert_eq!(event.old_expires_at, 10_000);
        assert_eq!(event.new_expires_at, 20_000);
        assert_eq!(event.timestamp, 30_000);
    }

    #[test]
    fn listing_updated_emitter_publishes_typed_event() {
        let env = Env::default();
        let contract_id = env.register(crate::AtomicMintContract, ());
        let seller = Address::generate(&env);

        env.as_contract(&contract_id, || {
            emit_listing_updated(&env, 7, 9, &seller, 100, 200, 300, 400, 500);
            let event = find_listing_updated_event(&env).expect("listing update event missing");
            assert_eq!(event.listing_id, 7);
            assert_eq!(event.token_id, 9);
            assert_eq!(event.seller, seller);
            assert_eq!(event.old_price, 100);
            assert_eq!(event.new_price, 200);
            assert_eq!(event.old_expires_at, 300);
            assert_eq!(event.new_expires_at, 400);
            assert_eq!(event.timestamp, 500);
        });
    }
}
