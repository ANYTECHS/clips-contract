use soroban_sdk::{symbol_short, Address, Env};
use crate::marketplace::types::{NftListedEvent, ListingCancelledEvent, NftSoldEvent, OfferCreatedEvent, OfferAcceptedEvent};
use crate::types::{ListingId, TokenId};

pub mod listing {
    use super::*;
    pub fn emit_listing_created(env: &Env, token_id: TokenId, seller: &Address, price: i128, payment_asset: &Address, _expiration: u64, timestamp: u64) {
        env.events().publish((symbol_short!("nft_list"),), NftListedEvent { listing_id: 0, token_id, seller: seller.clone(), price, asset: payment_asset.clone(), timestamp });
    }
    pub fn emit_listing_cancelled(env: &Env, token_id: TokenId, seller: &Address, _caller: &Address, timestamp: u64) {
        env.events().publish((symbol_short!("lst_cancl"),), ListingCancelledEvent { listing_id: 0, token_id, seller: seller.clone(), timestamp });
    }
    pub fn emit_listing_updated(env: &Env, listing_id: ListingId, token_id: TokenId, seller: &Address, payment_asset: &Address, _old_price: i128, new_price: i128, _old_expiration: u64, _new_expiration: u64, timestamp: u64) {
        env.events().publish((symbol_short!("nft_list"),), NftListedEvent { listing_id, token_id, seller: seller.clone(), price: new_price, asset: payment_asset.clone(), timestamp });
    }
    pub fn emit_nft_sold(env: &Env, token_id: TokenId, seller: &Address, buyer: &Address, price: i128, payment_asset: &Address, timestamp: u64) {
        env.events().publish((symbol_short!("nft_sold"),), NftSoldEvent { listing_id: 0, token_id, buyer: buyer.clone(), seller: seller.clone(), sale_amount: price, payment_asset: payment_asset.clone(), timestamp });
    }
}

pub mod offer {
    use super::*;
    pub fn emit_offer_made(env: &Env, token_id: TokenId, offerer: &Address, price: i128, payment_asset: &Address, expiration: u64, _timestamp: u64) {
        env.events().publish((symbol_short!("offr_crea"),), OfferCreatedEvent { offer_id: 0, token_id, buyer: offerer.clone(), offer_amount: price, asset: payment_asset.clone(), expiration });
    }
    pub fn emit_offer_accepted(env: &Env, token_id: TokenId, seller: &Address, offerer: &Address, price: i128, _payment_asset: &Address, timestamp: u64) {
        env.events().publish((symbol_short!("ofr_accpt"),), OfferAcceptedEvent { offer_id: 0, token_id, buyer: offerer.clone(), seller: seller.clone(), accepted_amount: price, timestamp });
    }
    pub fn emit_offer_cancelled(_env: &Env, _token_id: TokenId, _offerer: &Address, _caller: &Address, _timestamp: u64) {
        // Mocked
    }
}
